//! Live source reads stay beneath an explicitly selected workspace root.
//! Roots are canonicalized by workspace selection, not while reading. Symlinks
//! in a resolved root or its descendants and non-regular files are rejected.
//! Validate opened handles, not a path before reopening it.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path};

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
    let file = open_components(root, &components)?;
    if !file.metadata()?.is_file() {
        return Err(unsafe_path());
    }
    Ok(file)
}

pub(crate) fn read_to_string(root: &Path, path: &Path) -> io::Result<String> {
    let mut content = String::new();
    open(root, path)?.read_to_string(&mut content)?;
    Ok(content)
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
fn open_components(root: &Path, components: &[&std::ffi::OsStr]) -> io::Result<File> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::OpenOptionsExt;

    // Traversing a known source path needs directory search permission, not
    // permission to list every ancestor (for example a shared execute-only dir).
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let directory_access = libc::O_PATH;
    #[cfg(target_vendor = "apple")]
    let directory_access = libc::O_SEARCH;
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    let directory_access = libc::O_RDONLY;

    // Start at the filesystem root, not the workspace pathname: a registered
    // workspace or one of its ancestors can also have become a symlink.
    // Workspace::resolve already handles explicitly selected root symlinks.
    let mut directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(directory_access | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open("/")?;
    let root_components = root
        .components()
        .filter_map(|component| match component {
            Component::RootDir => None,
            Component::Normal(name) => Some(Ok(name)),
            _ => Some(Err(unsafe_path())),
        })
        .collect::<io::Result<Vec<_>>>()?;
    let count = root_components.len() + components.len();
    for (index, component) in root_components
        .into_iter()
        .chain(components.iter().copied())
        .enumerate()
    {
        let name = CString::new(component.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in file path"))?;
        let access = if index + 1 < count || components.is_empty() {
            directory_access | libc::O_DIRECTORY
        } else {
            // A substituted FIFO must not block before the regular-file check.
            libc::O_RDONLY | libc::O_NONBLOCK
        };
        let flags = access | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        // SAFETY: directory is live, name is NUL-terminated, and no creation
        // flags are used. A successful descriptor is immediately owned by File.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a new, uniquely owned descriptor.
        directory = unsafe { File::from_raw_fd(descriptor) };
    }
    Ok(directory)
}

#[cfg(windows)]
fn open_components(root: &Path, components: &[&std::ffi::OsStr]) -> io::Result<File> {
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
fn open_components(_root: &Path, _components: &[&std::ffi::OsStr]) -> io::Result<File> {
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
}
