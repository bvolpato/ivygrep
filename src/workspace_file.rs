//! Live source reads stay beneath an explicitly selected workspace root.
//! Roots are canonicalized by workspace selection, not while reading. Symlinks
//! in a resolved root or its descendants and non-regular files are rejected.
//! Validate opened handles, not a path before reopening it.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub(crate) fn validate_root(root: &Path) -> io::Result<()> {
    open_root(root).map(drop)
}

pub(crate) fn open_root(root: &Path) -> io::Result<File> {
    if !root.is_absolute() {
        return Err(unsafe_path());
    }
    let directory = open_components(root, &[])?;
    if !directory.metadata()?.is_dir() {
        return Err(unsafe_path());
    }
    Ok(directory)
}

pub(crate) fn open(root: &Path, path: &Path) -> io::Result<File> {
    if !root.is_absolute() {
        return Err(unsafe_path());
    }
    let components = relative_components(root, path)?;
    regular_file(open_components(root, &components)?)
}

/// Names of `path` beneath `root`. An absolute `path` must start with `root`.
fn relative_components<'a>(root: &Path, path: &'a Path) -> io::Result<Vec<&'a OsStr>> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root).map_err(|_| unsafe_path())?
    } else {
        path
    };
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name),
            _ => Err(unsafe_path()),
        })
        .collect::<io::Result<Vec<_>>>()?;
    if components.is_empty() {
        return Err(unsafe_path());
    }
    Ok(components)
}

fn regular_file(file: File) -> io::Result<File> {
    if !file.metadata()?.is_file() {
        return Err(unsafe_path());
    }
    Ok(file)
}

/// Reads many files beneath one workspace root during a single operation.
///
/// [`open`] walks every directory from the filesystem root for each file.
/// This handle does that walk once, for its first read. Each read then walks
/// only the names beneath the root, with the same checks as [`open`].
///
/// Create one handle for each operation, such as one search request or one
/// indexing batch. Do not keep a handle for a later operation: that operation
/// must validate the root again, so that it sees a replaced workspace. If the
/// root or an ancestor is replaced while a handle is in use, the handle
/// continues to read the directory that it validated.
pub(crate) struct RootHandle {
    root: PathBuf,
    // `None` if the root did not validate. Reads then use `open`, which
    // reports the same errors as a read without a handle.
    #[cfg(unix)]
    directory: std::sync::OnceLock<Option<File>>,
}

impl RootHandle {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            #[cfg(unix)]
            directory: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn open(&self, path: &Path) -> io::Result<File> {
        #[cfg(unix)]
        if let Some(directory) = self.directory.get_or_init(|| open_root(&self.root).ok()) {
            let components = relative_components(&self.root, path)?;
            return regular_file(open_beneath(directory, &components)?);
        }
        // Windows keeps its traversal handles only while one file is opened.
        open(&self.root, path)
    }

    /// Invalid UTF-8 is replaced, matching how indexing decodes source files.
    pub(crate) fn read_to_string(&self, path: &Path) -> io::Result<String> {
        Ok(lossy_string(self.read(path)?))
    }

    pub(crate) fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.open(path)?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

/// Invalid UTF-8 is replaced, matching how indexing decodes source files.
pub(crate) fn read_to_string(root: &Path, path: &Path) -> io::Result<String> {
    Ok(lossy_string(read(root, path)?))
}

pub(crate) fn lossy_string(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

pub(crate) fn read(root: &Path, path: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open(root, path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn unsafe_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "not a regular workspace file",
    )
}

#[cfg(unix)]
fn open_components(root: &Path, components: &[&OsStr]) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    // Start at the filesystem root, not the workspace pathname: a registered
    // workspace or one of its ancestors can also have become a symlink.
    // Workspace::resolve already handles explicitly selected root symlinks.
    let mut directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(directory_flags())
        .open("/")?;
    for component in root.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => directory = open_at(&directory, name, directory_flags())?,
            _ => return Err(unsafe_path()),
        }
    }
    if components.is_empty() {
        return Ok(directory);
    }
    open_beneath(&directory, components)
}

/// Opens the file that `components` name beneath an open `directory`. Every
/// name except the last must be a directory, and no name may be a symlink.
#[cfg(unix)]
fn open_beneath(directory: &File, components: &[&OsStr]) -> io::Result<File> {
    let (file_name, directories) = components.split_last().ok_or_else(unsafe_path)?;
    let mut parent = None;
    for name in directories {
        parent = Some(open_at(
            parent.as_ref().unwrap_or(directory),
            name,
            directory_flags(),
        )?);
    }
    // A substituted FIFO must not block before the regular-file check.
    let flags = libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    open_at(parent.as_ref().unwrap_or(directory), file_name, flags)
}

#[cfg(unix)]
fn directory_flags() -> libc::c_int {
    // Traversing a known source path needs directory search permission, not
    // permission to list every ancestor (for example a shared execute-only dir).
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let directory_access = libc::O_PATH;
    #[cfg(target_vendor = "apple")]
    let directory_access = libc::O_SEARCH;
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    let directory_access = libc::O_RDONLY;
    directory_access | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW
}

#[cfg(unix)]
fn open_at(directory: &File, name: &OsStr, flags: libc::c_int) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in file path"))?;
    // SAFETY: directory is live, name is NUL-terminated, and no creation
    // flags are used. A successful descriptor is immediately owned by File.
    let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openat returned a new, uniquely owned descriptor.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

#[cfg(windows)]
fn open_components(root: &Path, components: &[&OsStr]) -> io::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE,
    };

    fn open_component(path: &Path, directory: bool) -> io::Result<File> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        if directory {
            options.access_mode(FILE_READ_ATTRIBUTES | FILE_TRAVERSE);
        }
        options
            // Keep every traversed directory open without delete sharing.
            // A checked component cannot be renamed/replaced during traversal.
            .share_mode(
                FILE_SHARE_READ | FILE_SHARE_WRITE | if directory { 0 } else { FILE_SHARE_DELETE },
            )
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }

    // Workspace selection resolved the root. Re-canonicalizing here would
    // trust a later replacement of that root or an ancestor with a junction.
    let mut path = std::path::PathBuf::new();
    let mut root_components = Vec::new();
    for component in root.components() {
        match component {
            Component::Prefix(_) | Component::RootDir if root_components.is_empty() => {
                path.push(component.as_os_str())
            }
            Component::Normal(name) => root_components.push(name),
            _ => return Err(unsafe_path()),
        }
    }
    let root_handle = open_component(&path, true)?;
    let metadata = root_handle.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(unsafe_path());
    }
    let mut directories = vec![root_handle];
    let count = root_components.len() + components.len();
    for (index, component) in root_components
        .into_iter()
        .chain(components.iter().copied())
        .enumerate()
    {
        // Windows alternate streams are not ordinary workspace source files.
        if component.to_string_lossy().contains(':') {
            return Err(unsafe_path());
        }
        path.push(component);
        let file = open_component(&path, index + 1 < count || components.is_empty())?;
        let metadata = file.metadata()?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(unsafe_path());
        }
        if index + 1 == count {
            return Ok(file);
        }
        if !metadata.is_dir() {
            return Err(unsafe_path());
        }
        directories.push(file);
    }
    directories.pop().ok_or_else(unsafe_path)
}

#[cfg(not(any(unix, windows)))]
fn open_components(_root: &Path, _components: &[&OsStr]) -> io::Result<File> {
    // Indexed text remains available on platforms without a safe live opener.
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "safe workspace reads unavailable",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_normal_files_but_rejects_escape_and_directories() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "ordinary source").unwrap();
        validate_root(&root).unwrap();
        assert!(validate_root(&root.join("src/lib.rs")).is_err());
        assert_eq!(
            read(&root, Path::new("src/lib.rs")).unwrap(),
            b"ordinary source"
        );
        assert_eq!(
            read_to_string(&root, Path::new("src/lib.rs")).unwrap(),
            "ordinary source"
        );
        for path in [
            Path::new("../outside"),
            Path::new("src/../src/lib.rs"),
            Path::new("src"),
        ] {
            assert!(open(&root, path).is_err(), "{}", path.display());
        }
        let outside = tempfile::NamedTempFile::new().unwrap();
        assert!(open(&root, outside.path()).is_err());
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    #[test]
    fn readable_files_under_execute_only_ancestors_remain_available() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let parent = fixture.path().canonicalize().unwrap().join("traverse-only");
        let root = parent.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("source.rs"), "inside").unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o100)).unwrap();
        let content = read_to_string(&root, Path::new("source.rs"));
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(content.unwrap(), "inside");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn opened_files_remain_readable_across_leaf_replacement() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let source = root.join("source.rs");
        std::fs::write(&source, "original").unwrap();
        let mut file = open(&root, Path::new("source.rs")).unwrap();
        std::fs::rename(&source, root.join("previous.rs")).unwrap();
        std::fs::write(&source, "replacement").unwrap();
        let mut original = String::new();
        file.read_to_string(&mut original).unwrap();
        assert_eq!(original, "original");
        assert_eq!(
            read_to_string(&root, Path::new("source.rs")).unwrap(),
            "replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_but_accepts_a_previously_resolved_root() {
        use std::os::unix::fs::symlink;
        let fixture = tempfile::tempdir().unwrap();
        let fixture_root = fixture.path().canonicalize().unwrap();
        let root = fixture_root.join("root");
        let outside = fixture_root.join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("safe.rs"), "inside").unwrap();
        std::fs::write(outside.join("secret.rs"), "outside").unwrap();
        symlink(outside.join("secret.rs"), root.join("direct.rs")).unwrap();
        symlink(&outside, root.join("parent")).unwrap();
        let alias = fixture_root.join("selected-root");
        symlink(&root, &alias).unwrap();
        assert!(open(&root, Path::new("direct.rs")).is_err());
        assert!(open(&root, Path::new("parent/secret.rs")).is_err());
        assert!(open(&alias, Path::new("safe.rs")).is_err());
        let selected_root = alias.canonicalize().unwrap();
        assert_eq!(
            read_to_string(&selected_root, Path::new("safe.rs")).unwrap(),
            "inside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn opened_file_is_not_redirected_by_parent_replacement() {
        use std::os::unix::fs::symlink;
        let fixture = tempfile::tempdir().unwrap();
        let fixture_root = fixture.path().canonicalize().unwrap();
        let root = fixture_root.join("root");
        let outside = fixture_root.join("outside");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("src/lib.rs"), "inside").unwrap();
        std::fs::write(outside.join("lib.rs"), "outside").unwrap();
        let mut file = open(&root, Path::new("src/lib.rs")).unwrap();
        std::fs::rename(root.join("src"), root.join("original")).unwrap();
        symlink(&outside, root.join("src")).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "inside");
        assert!(open(&root, Path::new("src/lib.rs")).is_err());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn root_handle_reads_match_path_based_reads() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src/nested")).unwrap();
        std::fs::write(root.join("top.rs"), "top level").unwrap();
        std::fs::write(root.join("src/nested/lib.rs"), b"invalid \xff utf8").unwrap();
        let files = RootHandle::new(&root);
        for path in [Path::new("top.rs"), Path::new("src/nested/lib.rs")] {
            assert_eq!(files.read(path).unwrap(), read(&root, path).unwrap());
            assert_eq!(
                files.read_to_string(path).unwrap(),
                read_to_string(&root, path).unwrap()
            );
            // Search passes absolute candidate paths beneath the root.
            assert_eq!(
                files.read(&root.join(path)).unwrap(),
                read(&root, path).unwrap()
            );
        }
        let outside = tempfile::NamedTempFile::new().unwrap();
        for path in [
            Path::new("../outside"),
            Path::new("src/../top.rs"),
            Path::new("src"),
            Path::new(""),
            Path::new("missing.rs"),
            outside.path(),
        ] {
            assert!(files.open(path).is_err(), "{}", path.display());
            assert!(open(&root, path).is_err(), "{}", path.display());
        }
    }

    #[test]
    fn root_handle_for_an_invalid_root_reports_errors_for_each_read() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        std::fs::write(root.join("source.rs"), "inside").unwrap();
        let name = Path::new("source.rs");
        assert!(RootHandle::new(Path::new("relative")).open(name).is_err());
        assert!(RootHandle::new(&root.join("missing")).open(name).is_err());
        assert!(RootHandle::new(&root.join("source.rs")).open(name).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn root_handle_rejects_symlinks_and_special_files_beneath_the_root() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;
        let fixture = tempfile::tempdir().unwrap();
        let fixture_root = fixture.path().canonicalize().unwrap();
        let root = fixture_root.join("root");
        let outside = fixture_root.join("outside");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("src/lib.rs"), "inside").unwrap();
        std::fs::write(outside.join("secret.rs"), "outside").unwrap();
        symlink(outside.join("secret.rs"), root.join("direct.rs")).unwrap();
        symlink(&outside, root.join("parent")).unwrap();
        let pipe = std::ffi::CString::new(root.join("pipe").as_os_str().as_bytes()).unwrap();
        // SAFETY: pipe is a NUL-terminated path inside the test fixture.
        assert_eq!(unsafe { libc::mkfifo(pipe.as_ptr(), 0o600) }, 0);

        let files = RootHandle::new(&root);
        assert_eq!(
            files.read_to_string(Path::new("src/lib.rs")).unwrap(),
            "inside"
        );
        assert!(files.open(Path::new("direct.rs")).is_err());
        assert!(files.open(Path::new("parent/secret.rs")).is_err());
        // A FIFO without a writer must be rejected without blocking the open.
        assert!(files.open(Path::new("pipe")).is_err());

        // A directory replaced by a symlink after the handle was created.
        std::fs::rename(root.join("src"), root.join("original")).unwrap();
        symlink(&outside, root.join("src")).unwrap();
        std::fs::write(outside.join("lib.rs"), "outside").unwrap();
        assert!(files.open(Path::new("src/lib.rs")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn root_handle_stays_in_the_validated_directory_after_root_replacement() {
        use std::os::unix::fs::symlink;
        let fixture = tempfile::tempdir().unwrap();
        let fixture_root = fixture.path().canonicalize().unwrap();
        let root = fixture_root.join("root");
        let outside = fixture_root.join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(root.join("source.rs"), "inside").unwrap();
        std::fs::write(outside.join("source.rs"), "outside").unwrap();
        std::fs::write(outside.join("secret.rs"), "outside only").unwrap();
        let name = Path::new("source.rs");

        let files = RootHandle::new(&root);
        assert_eq!(files.read_to_string(name).unwrap(), "inside");
        std::fs::rename(&root, fixture_root.join("previous")).unwrap();
        symlink(&outside, &root).unwrap();

        // The handle never follows the replacement to the outside directory.
        assert_eq!(files.read_to_string(name).unwrap(), "inside");
        assert!(files.open(Path::new("secret.rs")).is_err());
        // The next operation validates the root again and rejects the symlink.
        assert!(RootHandle::new(&root).open(name).is_err());
        assert!(open(&root, name).is_err());

        // A directory recreated at the root path is read by the next operation.
        std::fs::remove_file(&root).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("source.rs"), "recreated").unwrap();
        assert_eq!(files.read_to_string(name).unwrap(), "inside");
        assert_eq!(
            RootHandle::new(&root).read_to_string(name).unwrap(),
            "recreated"
        );
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    #[test]
    fn root_handle_reads_files_under_execute_only_ancestors() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let parent = fixture.path().canonicalize().unwrap().join("traverse-only");
        let root = parent.join("workspace");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/source.rs"), "inside").unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o100)).unwrap();
        let content = RootHandle::new(&root).read_to_string(Path::new("src/source.rs"));
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(content.unwrap(), "inside");
    }
}
