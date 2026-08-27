use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use lru::LruCache;
use parking_lot::Mutex;

use super::presentation::{LineSpan, line_spans};

const MAX_CACHED_FILES: usize = 256;
const MAX_CACHED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl FileStamp {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

#[derive(Clone)]
pub(super) struct CachedFileContent {
    stamp: FileStamp,
    pub(super) content: Arc<str>,
    pub(super) lines: Arc<[LineSpan]>,
}

impl CachedFileContent {
    fn bytes(&self, path: &Path) -> usize {
        self.content.len()
            + std::mem::size_of_val(self.lines.as_ref())
            + path.as_os_str().len()
            + std::mem::size_of::<Self>()
    }
}

pub(super) struct FileContentCache {
    entries: LruCache<PathBuf, CachedFileContent>,
    bytes: usize,
    max_bytes: usize,
}

impl FileContentCache {
    fn new(max_files: NonZeroUsize, max_bytes: usize) -> Self {
        Self {
            entries: LruCache::new(max_files),
            bytes: 0,
            max_bytes,
        }
    }

    pub(super) fn shared() -> Arc<Mutex<Self>> {
        static CACHE: OnceLock<Arc<Mutex<FileContentCache>>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                Arc::new(Mutex::new(Self::new(
                    NonZeroUsize::new(MAX_CACHED_FILES).unwrap(),
                    MAX_CACHED_BYTES,
                )))
            })
            .clone()
    }

    fn remove(&mut self, path: &Path) {
        if let Some(content) = self.entries.pop(path) {
            self.bytes -= content.bytes(path);
        }
    }

    fn insert(&mut self, path: PathBuf, content: CachedFileContent) {
        self.remove(&path);
        let bytes = content.bytes(&path);
        if bytes > self.max_bytes {
            return;
        }
        self.bytes += bytes;
        if let Some((path, content)) = self.entries.push(path, content) {
            self.bytes -= content.bytes(&path);
        }
        while self.bytes > self.max_bytes {
            if let Some((path, content)) = self.entries.pop_lru() {
                self.bytes -= content.bytes(&path);
            }
        }
    }

    pub(super) fn read(cache: &Mutex<Self>, path: &Path) -> Option<CachedFileContent> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => {
                cache.lock().remove(path);
                return None;
            }
        };
        let stamp = FileStamp::from_metadata(&metadata);
        {
            let mut cache = cache.lock();
            if let Some(content) = cache.entries.get(path)
                && content.stamp == stamp
            {
                return Some(content.clone());
            }
            cache.remove(path);
        }

        // Disk reads and line indexing stay outside the shared cache lock.
        let content = Arc::<str>::from(fs::read_to_string(path).ok()?);
        let lines = line_spans(&content).into();
        let cached = CachedFileContent {
            stamp,
            content,
            lines,
        };
        cache.lock().insert(path.to_path_buf(), cached.clone());
        Some(cached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_budget_counts_line_spans_and_evicts_the_least_recent_file() {
        let root = tempfile::tempdir().unwrap();
        for name in ["a", "b", "c"] {
            fs::write(root.path().join(name), "line\n".repeat(100)).unwrap();
        }
        let cache = Mutex::new(FileContentCache::new(
            NonZeroUsize::new(3).unwrap(),
            usize::MAX,
        ));
        let a = root.path().join("a");
        let b = root.path().join("b");
        let c = root.path().join("c");
        let first = FileContentCache::read(&cache, &a).unwrap();
        let entry_bytes = first.bytes(&a);
        assert!(entry_bytes > first.content.len());
        cache.lock().max_bytes = entry_bytes * 2;
        FileContentCache::read(&cache, &b).unwrap();
        let hit = FileContentCache::read(&cache, &a).unwrap();
        assert!(Arc::ptr_eq(&first.content, &hit.content));
        FileContentCache::read(&cache, &c).unwrap();
        let cache = cache.lock();
        assert!(cache.entries.contains(&a));
        assert!(!cache.entries.contains(&b));
        assert!(cache.entries.contains(&c));
        assert_eq!(cache.bytes, entry_bytes * 2);
    }

    #[test]
    fn oversized_replacement_is_returned_without_retaining_stale_content() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.rs");
        fs::write(&path, "fn original() {}\n").unwrap();
        let cache = Mutex::new(FileContentCache::new(NonZeroUsize::new(2).unwrap(), 1024));
        FileContentCache::read(&cache, &path).unwrap();
        fs::write(&path, "large\n".repeat(1024)).unwrap();
        let updated = FileContentCache::read(&cache, &path).unwrap();
        assert_eq!(updated.lines.len(), 1024);
        assert!(cache.lock().entries.is_empty());
        assert_eq!(cache.lock().bytes, 0);
    }

    #[cfg(unix)]
    #[test]
    fn replacement_with_preserved_mtime_invalidates_shared_content() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source.rs");
        fs::write(&path, "old\n").unwrap();
        let cache = Mutex::new(FileContentCache::new(NonZeroUsize::new(2).unwrap(), 1024));
        FileContentCache::read(&cache, &path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let replacement = root.path().join("replacement");
        fs::write(&replacement, "new\n").unwrap();
        fs::File::open(&replacement)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        fs::rename(replacement, &path).unwrap();
        assert_eq!(
            &*FileContentCache::read(&cache, &path).unwrap().content,
            "new\n"
        );
    }
}
