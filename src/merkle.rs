use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::chunking::is_indexable_file;
use crate::workspace::index_path_string;

use std::io::IsTerminal;

const MAX_INDEXABLE_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn normalized_indexable_content<'a>(path: &Path, bytes: &'a [u8]) -> Cow<'a, [u8]> {
    if !is_indexable_file(path, bytes) || !bytes.windows(2).any(|window| window == b"\r\n") {
        return Cow::Borrowed(bytes);
    }

    let mut normalized = Vec::with_capacity(bytes.len());
    let mut start = 0;
    for (index, window) in bytes.windows(2).enumerate() {
        if window == b"\r\n" {
            normalized.extend_from_slice(&bytes[start..index]);
            start = index + 1;
        }
    }
    normalized.extend_from_slice(&bytes[start..]);
    Cow::Owned(normalized)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerkleSnapshot {
    pub root_hash: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MerkleDiff {
    pub added_or_modified: Vec<(PathBuf, bool)>,
    pub deleted: Vec<PathBuf>,
}

/// Identity of one snapshot file's content: size, modification time, and on
/// Unix the inode plus change time. Size and mtime alone can survive an
/// mtime-preserving restore or a same-length overwrite on a coarse-timestamp
/// filesystem; ctime changes on every write and cannot be set by userland, and
/// the inode changes when the file is replaced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SnapshotStamp {
    len: u64,
    modified_nanos: u128,
    inode: u64,
    changed_nanos: i128,
}

fn snapshot_stamp(path: &Path) -> Option<SnapshotStamp> {
    let metadata = fs::metadata(path).ok()?;
    let modified_nanos = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    #[cfg(unix)]
    let (inode, changed_nanos) = {
        use std::os::unix::fs::MetadataExt;
        (
            metadata.ino(),
            i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()),
        )
    };
    #[cfg(not(unix))]
    let (inode, changed_nanos) = (0u64, 0i128);
    Some(SnapshotStamp {
        len: metadata.len(),
        modified_nanos,
        inode,
        changed_nanos,
    })
}

fn verified_stamp_path(path: &Path) -> PathBuf {
    path.with_extension("verified")
}

fn read_verified_stamp(path: &Path) -> Option<SnapshotStamp> {
    let raw = fs::read_to_string(verified_stamp_path(path)).ok()?;
    let mut fields = raw.split_whitespace();
    let stamp = SnapshotStamp {
        len: fields.next()?.parse().ok()?,
        modified_nanos: fields.next()?.parse().ok()?,
        inode: fields.next()?.parse().ok()?,
        changed_nanos: fields.next()?.parse().ok()?,
    };
    fields.next().is_none().then_some(stamp)
}

fn write_verified_stamp(path: &Path, stamp: SnapshotStamp) {
    let sidecar = verified_stamp_path(path);
    let tmp = sidecar.with_extension("verified.tmp");
    let payload = format!(
        "{} {} {} {}\n",
        stamp.len, stamp.modified_nanos, stamp.inode, stamp.changed_nanos
    );
    if fs::write(&tmp, payload).is_ok() {
        let _ = fs::rename(&tmp, &sidecar);
    }
}

impl MerkleSnapshot {
    pub fn empty() -> Self {
        Self {
            root_hash: String::new(),
            files: BTreeMap::new(),
        }
    }

    /// True if the snapshot file exists but cannot be parsed. Such a file makes
    /// an incremental diff impossible (we'd diff against an empty old set, which
    /// re-adds everything but never removes chunks for files deleted before the
    /// corruption), so the caller should force a full rebuild instead.
    ///
    /// Parsing a large snapshot on every health check is expensive (the file
    /// grows with the repository), so a successful parse or save records the
    /// file's size and modification time in a sidecar; while that stamp
    /// matches, the content is known good and the parse is skipped.
    pub fn file_is_corrupt(path: &Path) -> bool {
        let Some(stamp) = snapshot_stamp(path) else {
            // Missing or unreadable (not corrupt content) — let normal IO
            // handling deal.
            return false;
        };
        if read_verified_stamp(path) == Some(stamp) {
            return false;
        }
        match fs::read(path) {
            Ok(data) => {
                let corrupt = serde_json::from_slice::<Self>(&data).is_err();
                if !corrupt {
                    write_verified_stamp(path, stamp);
                }
                corrupt
            }
            Err(_) => false,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        // Propagate genuine IO errors (permissions, disk) rather than masking
        // them as an empty snapshot. Only *corrupt content* (a truncated/garbage
        // file, typically from a crash mid-write) falls back to empty; that case
        // is also flagged by quick_index_health (file_is_corrupt), which forces
        // a full rebuild that clears orphaned chunks — so we don't wedge
        // incremental indexing forever on every reindex/watcher tick.
        let data = fs::read(path)?;
        match serde_json::from_slice(&data) {
            Ok(snapshot) => Ok(snapshot),
            Err(_) => Ok(Self::empty()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let payload = serde_json::to_vec(self)?;
        // Atomic write: write to a sibling tmp file, then rename.
        // rename() is atomic on POSIX when src and dst are on the same
        // filesystem (guaranteed here since tmp is in the same directory).
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, payload)?;
        fs::rename(&tmp, path)?;
        if let Some(stamp) = snapshot_stamp(path) {
            write_verified_stamp(path, stamp);
        }
        Ok(())
    }

    pub fn build(root: &Path, skip_gitignore: bool) -> Result<Self> {
        Self::build_inner(root, false, skip_gitignore)
    }

    /// Build a snapshot using content-based hashing (reads file contents instead of mtime).
    /// This is slower but produces identical hashes for identical files across worktrees,
    /// enabling correct delta computation when seeding a worktree from a base index.
    pub fn build_content_based(root: &Path, skip_gitignore: bool) -> Result<Self> {
        Self::build_inner(root, true, skip_gitignore)
    }

    pub(crate) fn path_matches_metadata_snapshot(path: &Path, snapshot_hash: &str) -> bool {
        let Some(expected_hash) = snapshot_hash
            .strip_suffix("-0")
            .or_else(|| snapshot_hash.strip_suffix("-1"))
        else {
            return false;
        };
        fs::metadata(path).ok().is_some_and(|metadata| {
            metadata.len() <= MAX_INDEXABLE_FILE_BYTES
                && metadata_file_hash(&metadata) == expected_hash
        })
    }

    fn build_inner(root: &Path, content_based: bool, skip_gitignore: bool) -> Result<Self> {
        // If skip_gitignore is true, do a fast standard walk first to record which files WOULD have been included properly.
        let unignored_paths = Arc::new(if skip_gitignore {
            let standard_walker = crate::walker::source_walker(root, false);
            let paths = std::sync::Mutex::new(std::collections::HashSet::new());
            let root_owned = root.to_path_buf();
            standard_walker.build_parallel().run(|| {
                let paths_ref = &paths;
                let root_ref = &root_owned;
                Box::new(move |entry| {
                    if let Ok(e) = entry
                        && e.file_type().is_some_and(|ft| ft.is_file())
                        && let Ok(rel) = e.path().strip_prefix(root_ref)
                    {
                        paths_ref.lock().unwrap().insert(index_path_string(rel));
                    }
                    ignore::WalkState::Continue
                })
            });
            paths.into_inner().unwrap()
        } else {
            std::collections::HashSet::new()
        });

        let walker = crate::walker::source_walker(root, skip_gitignore);

        let show_progress = std::io::stderr().is_terminal();
        let scanned = AtomicUsize::new(0);

        // Each worker thread collects entries into its own buffer with zero
        // lock contention. A FlushGuard wraps the buffer and flushes it into
        // the shared Vec exactly once when the walker drops the per-thread
        // closure (thread exit). This reduces Mutex acquisitions from N (one
        // per file) to T (one per thread, typically 4-8).
        let all_pairs: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
        let root_owned = root.to_path_buf();

        struct FlushGuard<'a> {
            buf: Vec<(String, String)>,
            target: &'a std::sync::Mutex<Vec<(String, String)>>,
        }
        impl Drop for FlushGuard<'_> {
            fn drop(&mut self) {
                if !self.buf.is_empty() {
                    self.target.lock().unwrap().append(&mut self.buf);
                }
            }
        }

        walker.build_parallel().run(|| {
            let root_ref = &root_owned;
            let scanned_ref = &scanned;
            let pairs_ref = &all_pairs;
            let unignored_paths_clone = Arc::clone(&unignored_paths);
            let mut guard = FlushGuard {
                buf: Vec::with_capacity(512),
                target: pairs_ref,
            };

            Box::new(move |entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return ignore::WalkState::Continue,
                };
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }

                let path = entry.path();
                let rel = match path.strip_prefix(root_ref) {
                    Ok(r) => r.to_path_buf(),
                    Err(_) => return ignore::WalkState::Continue,
                };

                let metadata = match fs::metadata(path) {
                    Ok(m) => m,
                    Err(_) => return ignore::WalkState::Continue,
                };
                if metadata.len() > MAX_INDEXABLE_FILE_BYTES {
                    return ignore::WalkState::Continue;
                }

                let n = scanned_ref.fetch_add(1, Ordering::Relaxed) + 1;
                if show_progress && n.is_multiple_of(5000) {
                    eprint!("\r\x1b[K  scanning files... {}", n);
                }

                let file_hash = if content_based {
                    let content = match fs::read(path) {
                        Ok(c) => c,
                        Err(_) => return ignore::WalkState::Continue,
                    };
                    let rel_str = index_path_string(&rel);
                    let content = normalized_indexable_content(path, &content);
                    let mut data = Vec::with_capacity(rel_str.len() + content.len());
                    data.extend_from_slice(rel_str.as_bytes());
                    data.extend_from_slice(content.as_ref());
                    hex::encode(xxhash_rust::xxh3::xxh3_128(&data).to_le_bytes())
                } else {
                    metadata_file_hash(&metadata)
                };

                let rel_str = index_path_string(&rel);
                let is_ignored = skip_gitignore && !unignored_paths_clone.contains(&rel_str);
                let final_hash = if is_ignored {
                    format!("{file_hash}-1")
                } else {
                    format!("{file_hash}-0")
                };

                guard.buf.push((rel_str, final_hash));
                ignore::WalkState::Continue
            })
        });

        if show_progress {
            eprint!("\r\x1b[K");
        }

        let files: BTreeMap<String, String> = all_pairs.into_inner().unwrap().into_iter().collect();
        let root_hash = root_hash(&files);
        Ok(Self { root_hash, files })
    }

    pub fn diff(&self, newer: &Self) -> MerkleDiff {
        if self.root_hash == newer.root_hash {
            return MerkleDiff::default();
        }

        let mut added_or_modified = Vec::new();
        let mut deleted = Vec::new();
        let mut old = self.files.iter().peekable();
        let mut new = newer.files.iter().peekable();

        while let (Some((old_path, old_hash)), Some((new_path, new_hash))) =
            (old.peek(), new.peek())
        {
            match old_path.cmp(new_path) {
                std::cmp::Ordering::Less => {
                    deleted.push(PathBuf::from(old_path.as_str()));
                    old.next();
                }
                std::cmp::Ordering::Equal => {
                    if old_hash != new_hash {
                        added_or_modified
                            .push((PathBuf::from(new_path.as_str()), new_hash.ends_with("-1")));
                    }
                    old.next();
                    new.next();
                }
                std::cmp::Ordering::Greater => {
                    added_or_modified
                        .push((PathBuf::from(new_path.as_str()), new_hash.ends_with("-1")));
                    new.next();
                }
            }
        }

        for (path, _) in old {
            deleted.push(PathBuf::from(path));
        }
        for (path, hash) in new {
            added_or_modified.push((PathBuf::from(path), hash.ends_with("-1")));
        }

        MerkleDiff {
            added_or_modified,
            deleted,
        }
    }

    /// Refresh a known set of changed files without walking the workspace.
    ///
    /// Returns `None` when a targeted refresh is unsafe and the caller should
    /// fall back to a full snapshot rebuild. Directory changes, new files, and
    /// ignore-rule edits require the full walker to classify affected paths.
    pub fn refresh_paths(
        &self,
        root: &Path,
        rel_paths: &[PathBuf],
        skip_gitignore: bool,
    ) -> Result<Option<(Self, MerkleDiff)>> {
        if skip_gitignore {
            return Ok(None);
        }

        let mut newer = self.clone();
        let mut changed = false;
        for rel_path in rel_paths {
            if rel_path.as_os_str().is_empty()
                || rel_path
                    .file_name()
                    .is_some_and(|name| name == ".gitignore" || name == ".ignore")
            {
                return Ok(None);
            }

            let path = root.join(rel_path);
            if path.is_dir() {
                return Ok(None);
            }

            let key = index_path_string(rel_path);
            let next_hash = match fs::metadata(&path) {
                Ok(metadata)
                    if metadata.is_file() && metadata.len() <= MAX_INDEXABLE_FILE_BYTES =>
                {
                    let is_ignored = newer
                        .files
                        .get(&key)
                        .is_some_and(|hash| hash.ends_with("-1"));
                    Some(format!(
                        "{}-{}",
                        metadata_file_hash(&metadata),
                        u8::from(is_ignored)
                    ))
                }
                Ok(metadata) if metadata.is_dir() => return Ok(None),
                _ => None,
            };

            match next_hash {
                Some(hash) => {
                    // New files must pass through the full ignore-aware walker.
                    // Existing files already have a visibility classification
                    // in this snapshot, so metadata-only refresh is safe.
                    if !newer.files.contains_key(&key) {
                        return Ok(None);
                    }
                    if newer.files.get(&key) != Some(&hash) {
                        newer.files.insert(key, hash);
                        changed = true;
                    }
                }
                None => {
                    let dir_prefix = format!("{key}/");
                    if newer
                        .files
                        .keys()
                        .any(|existing| existing.starts_with(&dir_prefix))
                    {
                        return Ok(None);
                    }
                    changed |= newer.files.remove(&key).is_some();
                }
            }
        }

        if changed {
            newer.root_hash = root_hash(&newer.files);
        }
        let diff = self.diff(&newer);
        Ok(Some((newer, diff)))
    }
}

fn metadata_file_hash(metadata: &fs::Metadata) -> String {
    // The relative path is already the map key and participates in root_hash(),
    // so the per-file value only needs metadata.
    let mut data = [0_u8; 40];
    let mut len = 0;
    data[len..len + 8].copy_from_slice(&metadata.len().to_le_bytes());
    len += 8;
    if let Ok(mtime) = metadata.modified()
        && let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH)
    {
        data[len..len + 16].copy_from_slice(&duration.as_nanos().to_le_bytes());
        len += 16;
    }
    // On macOS and Linux, ctime changes when contents or inode metadata change
    // even when a caller restores mtime.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        data[len..len + 8].copy_from_slice(&metadata.ctime().to_le_bytes());
        data[len + 8..len + 16].copy_from_slice(&metadata.ctime_nsec().to_le_bytes());
        len += 16;
    }
    hex::encode(xxhash_rust::xxh3::xxh3_128(&data[..len]).to_le_bytes())
}

fn root_hash(files: &BTreeMap<String, String>) -> String {
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    for (path, hash) in files {
        hasher.update(path.as_bytes());
        hasher.update(hash.as_bytes());
    }
    hex::encode(hasher.digest128().to_le_bytes())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn streaming_root_hash_matches_contiguous_hash() {
        let files = BTreeMap::from([
            ("README.md".to_string(), "0123456789abcdef".to_string()),
            ("src/lib.rs".to_string(), "fedcba9876543210".to_string()),
            (
                "unicode/ivy-\u{1f33f}.rs".to_string(),
                "00ff00ff".to_string(),
            ),
        ]);
        let contiguous: Vec<u8> = files
            .iter()
            .flat_map(|(path, hash)| path.bytes().chain(hash.bytes()))
            .collect();

        assert_eq!(
            root_hash(&files),
            hex::encode(xxhash_rust::xxh3::xxh3_128(&contiguous).to_le_bytes())
        );
    }

    #[test]
    fn corruption_check_is_memoized_by_verified_stamp() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("merkle_snapshot.json");
        let mut snapshot = MerkleSnapshot::empty();
        snapshot
            .files
            .insert("src/lib.rs".to_string(), "hash-0".to_string());
        snapshot.save(&path).unwrap();

        assert!(
            verified_stamp_path(&path).exists(),
            "save records the stamp"
        );
        assert!(!MerkleSnapshot::file_is_corrupt(&path));

        // Truncate in place: the size changes, so the stamp no longer matches
        // and the content is parsed again.
        let data = fs::read(&path).unwrap();
        fs::write(&path, &data[..data.len() / 2]).unwrap();
        assert!(MerkleSnapshot::file_is_corrupt(&path));

        // Restoring valid content re-verifies and re-records the stamp.
        fs::remove_file(verified_stamp_path(&path)).unwrap();
        fs::write(&path, &data).unwrap();
        assert!(!MerkleSnapshot::file_is_corrupt(&path));
        assert_eq!(read_verified_stamp(&path), snapshot_stamp(&path));

        // A same-length corruption that preserves the mtime (as an
        // mtime-preserving restore would) still invalidates the stamp on Unix
        // because the change time and, after a replace, the inode differ.
        #[cfg(unix)]
        {
            let modified = fs::metadata(&path).unwrap().modified().unwrap();
            let mut garbage = data.clone();
            garbage[0] = b'#';
            let replacement = path.with_extension("replacement");
            fs::write(&replacement, &garbage).unwrap();
            fs::File::open(&replacement)
                .unwrap()
                .set_modified(modified)
                .unwrap();
            fs::rename(&replacement, &path).unwrap();
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
            assert!(
                MerkleSnapshot::file_is_corrupt(&path),
                "same length and mtime must not bypass validation"
            );
        }
    }

    #[test]
    fn load_corrupt_snapshot_falls_back_to_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("merkle_snapshot.json");
        fs::write(&path, b"{ this is not valid json").unwrap();
        // Must not error (which would wedge incremental indexing forever) —
        // fall back to empty so the next diff does a clean full reindex.
        let snap = MerkleSnapshot::load(&path).unwrap();
        assert!(snap.files.is_empty());
        assert_eq!(snap.root_hash, MerkleSnapshot::empty().root_hash);
    }

    #[test]
    fn merkle_diff_detects_add_modify_delete() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
        fs::write(root.join("b.py"), "def b():\n    pass\n").unwrap();

        let first = MerkleSnapshot::build(root, false).unwrap();

        fs::remove_file(root.join("a.rs")).unwrap();
        fs::write(root.join("b.py"), "def b():\n    return 1\n").unwrap();
        fs::write(root.join("c.ts"), "export function c() {}\n").unwrap();

        let second = MerkleSnapshot::build(root, false).unwrap();
        let diff = first.diff(&second);

        assert!(diff.deleted.contains(&PathBuf::from("a.rs")));
        assert!(
            diff.added_or_modified
                .contains(&(PathBuf::from("b.py"), false))
        );
        assert!(
            diff.added_or_modified
                .contains(&(PathBuf::from("c.ts"), false))
        );
    }

    #[test]
    fn targeted_refresh_detects_file_modify_delete() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
        let first = MerkleSnapshot::build(root, false).unwrap();

        fs::write(root.join("a.rs"), "fn a() { println!(\"changed\"); }\n").unwrap();
        let (second, diff) = first
            .refresh_paths(root, &[PathBuf::from("a.rs")], false)
            .unwrap()
            .unwrap();
        assert!(
            diff.added_or_modified
                .contains(&(PathBuf::from("a.rs"), false))
        );

        fs::remove_file(root.join("a.rs")).unwrap();
        let (_, diff) = second
            .refresh_paths(root, &[PathBuf::from("a.rs")], false)
            .unwrap()
            .unwrap();
        assert_eq!(diff.deleted, vec![PathBuf::from("a.rs")]);
    }

    #[test]
    fn targeted_refresh_preserves_existing_visibility_marker() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
        let mut snapshot = MerkleSnapshot::build(root, false).unwrap();
        let hash = snapshot.files["a.rs"].trim_end_matches("-0").to_string();
        snapshot
            .files
            .insert("a.rs".to_string(), format!("{hash}-1"));
        snapshot.root_hash = root_hash(&snapshot.files);

        fs::write(root.join("a.rs"), "fn a() { println!(\"changed\"); }\n").unwrap();
        let (_, diff) = snapshot
            .refresh_paths(root, &[PathBuf::from("a.rs")], false)
            .unwrap()
            .unwrap();
        assert_eq!(diff.added_or_modified, vec![(PathBuf::from("a.rs"), true)]);
    }

    #[test]
    fn targeted_refresh_falls_back_for_new_file_classification() {
        let dir = tempdir().unwrap();
        let snapshot = MerkleSnapshot::empty();
        fs::write(dir.path().join("new.rs"), "fn new() {}\n").unwrap();

        assert!(
            snapshot
                .refresh_paths(dir.path(), &[PathBuf::from("new.rs")], false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn targeted_refresh_falls_back_for_deleted_directory() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("nested/file.rs"), "fn nested() {}\n").unwrap();
        let snapshot = MerkleSnapshot::build(dir.path(), false).unwrap();
        fs::remove_dir_all(dir.path().join("nested")).unwrap();

        assert!(
            snapshot
                .refresh_paths(dir.path(), &[PathBuf::from("nested")], false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn targeted_refresh_falls_back_for_ignore_rule_changes() {
        let dir = tempdir().unwrap();
        let snapshot = MerkleSnapshot::empty();
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();

        assert!(
            snapshot
                .refresh_paths(dir.path(), &[PathBuf::from(".gitignore")], false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn content_based_snapshot_ignores_source_line_ending_style() {
        let left = tempdir().unwrap();
        let right = tempdir().unwrap();
        fs::write(
            left.path().join("lib.rs"),
            "pub fn answer() -> u8 {\n    42\n}\n",
        )
        .unwrap();
        fs::write(
            right.path().join("lib.rs"),
            "pub fn answer() -> u8 {\r\n    42\r\n}\r\n",
        )
        .unwrap();

        assert_eq!(
            MerkleSnapshot::build_content_based(left.path(), false).unwrap(),
            MerkleSnapshot::build_content_based(right.path(), false).unwrap()
        );
    }

    #[test]
    fn snapshot_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.json");

        let mut map = BTreeMap::new();
        map.insert("a.rs".to_string(), "hash1".to_string());

        let snapshot = MerkleSnapshot {
            root_hash: "root".to_string(),
            files: map,
        };

        snapshot.save(&path).unwrap();
        let loaded = MerkleSnapshot::load(&path).unwrap();
        assert_eq!(snapshot, loaded);

        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"\n").unwrap();
    }

    #[test]
    fn medium_txt_files_are_included_in_snapshot() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let content = "lorem ipsum dolor sit amet\n".repeat(160_000);

        fs::write(root.join("shakespeare.txt"), content).unwrap();

        let snapshot = MerkleSnapshot::build(root, false).unwrap();
        assert!(snapshot.files.contains_key("shakespeare.txt"));
    }

    #[test]
    fn unknown_text_files_are_included_in_snapshot() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("notes.custom"),
            "plain text in custom extension\n",
        )
        .unwrap();
        // Binary files are included in the Merkle snapshot for change detection.
        // Actual binary filtering happens at chunking time, not scan time.
        fs::write(root.join("blob.custom"), b"\x89PNG\r\n\x1a\n\0\0\0IHDR").unwrap();

        let snapshot = MerkleSnapshot::build(root, false).unwrap();
        assert!(snapshot.files.contains_key("notes.custom"));
        assert!(snapshot.files.contains_key("blob.custom"));
    }

    #[test]
    fn dot_git_directory_is_excluded_but_other_hidden_files_are_included() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Simulate a .git directory with objects
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            root.join(".git/objects/pack.idx"),
            "fake pack index content\n",
        )
        .unwrap();

        // Regular hidden files that SHOULD be indexed
        fs::write(root.join(".env"), "DATABASE_URL=postgres://localhost\n").unwrap();
        fs::write(root.join(".eslintrc.json"), "{}\n").unwrap();

        // Normal source file
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();

        let snapshot = MerkleSnapshot::build(root, false).unwrap();

        // .git contents must be excluded
        assert!(!snapshot.files.contains_key(".git/HEAD"));
        assert!(!snapshot.files.contains_key(".git/objects/pack.idx"));
        assert!(
            snapshot.files.keys().all(|k| !k.starts_with(".git/")),
            "no file under .git/ should be indexed"
        );

        // Other hidden files and normal files must be included
        assert!(snapshot.files.contains_key(".env"));
        assert!(snapshot.files.contains_key(".eslintrc.json"));
        assert!(snapshot.files.contains_key("main.rs"));
    }
}
