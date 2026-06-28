#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::{Component, Path, PathBuf};

pub(crate) fn available_memory_bytes() -> Option<u64> {
    platform_available_memory_bytes()
}

#[cfg(target_os = "linux")]
fn platform_available_memory_bytes() -> Option<u64> {
    let host_available = fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| parse_meminfo_bytes(&contents, "MemAvailable:"));
    let cgroup_available = cgroup_available_memory_bytes();

    match (host_available, cgroup_available) {
        (Some(host), Some(cgroup)) => Some(host.min(cgroup)),
        (Some(host), None) => Some(host),
        (None, Some(cgroup)) => Some(cgroup),
        (None, None) => None,
    }
}

#[cfg(target_os = "linux")]
fn cgroup_available_memory_bytes() -> Option<u64> {
    let cgroup = fs::read_to_string("/proc/self/cgroup").ok();
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok();

    let v2_available = cgroup
        .as_deref()
        .and_then(|contents| parse_cgroup_path(contents, None))
        .zip(
            mountinfo
                .as_deref()
                .and_then(|contents| parse_cgroup_mount(contents, "cgroup2", None)),
        )
        .and_then(|(path, mount)| {
            cgroup_hierarchy_available(
                &resolve_cgroup_dir(&mount, &path)?,
                &mount.mount_point,
                "memory.max",
                "memory.current",
            )
        });
    if v2_available.is_some() {
        return v2_available;
    }

    let v1_available = cgroup
        .as_deref()
        .and_then(|contents| parse_cgroup_path(contents, Some("memory")))
        .zip(
            mountinfo
                .as_deref()
                .and_then(|contents| parse_cgroup_mount(contents, "cgroup", Some("memory"))),
        )
        .and_then(|(path, mount)| {
            cgroup_hierarchy_available(
                &resolve_cgroup_dir(&mount, &path)?,
                &mount.mount_point,
                "memory.limit_in_bytes",
                "memory.usage_in_bytes",
            )
        });
    v1_available
        .or_else(|| {
            cgroup_available_from_paths(
                "/sys/fs/cgroup/memory.max",
                "/sys/fs/cgroup/memory.current",
            )
        })
        .or_else(|| {
            cgroup_available_from_paths(
                "/sys/fs/cgroup/memory/memory.limit_in_bytes",
                "/sys/fs/cgroup/memory/memory.usage_in_bytes",
            )
        })
}

#[cfg(target_os = "linux")]
fn cgroup_available_from_paths(
    limit_path: impl AsRef<Path>,
    usage_path: impl AsRef<Path>,
) -> Option<u64> {
    let limit = fs::read_to_string(limit_path)
        .ok()
        .and_then(|value| parse_cgroup_limit(&value))?;
    let usage = fs::read_to_string(usage_path)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(limit.saturating_sub(usage))
}

#[cfg(target_os = "linux")]
#[derive(Debug, Eq, PartialEq)]
struct CgroupMount {
    root: PathBuf,
    mount_point: PathBuf,
}

#[cfg(target_os = "linux")]
fn parse_cgroup_path(contents: &str, controller: Option<&str>) -> Option<String> {
    contents.lines().find_map(|line| {
        let mut fields = line.splitn(3, ':');
        let _hierarchy = fields.next()?;
        let controllers = fields.next()?;
        let path = fields.next()?;
        let matches = match controller {
            Some(controller) => controllers.split(',').any(|value| value == controller),
            None => controllers.is_empty(),
        };
        matches.then(|| path.to_string())
    })
}

#[cfg(target_os = "linux")]
fn parse_cgroup_mount(
    contents: &str,
    filesystem: &str,
    controller: Option<&str>,
) -> Option<CgroupMount> {
    contents.lines().find_map(|line| {
        let (mount_fields, filesystem_fields) = line.split_once(" - ")?;
        let mut mount_fields = mount_fields.split_whitespace();
        let root = mount_fields.nth(3)?;
        let mount_point = mount_fields.next()?;

        let mut filesystem_fields = filesystem_fields.split_whitespace();
        if filesystem_fields.next()? != filesystem {
            return None;
        }
        let _source = filesystem_fields.next()?;
        let options = filesystem_fields.next().unwrap_or_default();
        if controller.is_some_and(|name| !options.split(',').any(|value| value == name)) {
            return None;
        }

        Some(CgroupMount {
            root: PathBuf::from(decode_mountinfo_path(root)),
            mount_point: PathBuf::from(decode_mountinfo_path(mount_point)),
        })
    })
}

#[cfg(target_os = "linux")]
fn decode_mountinfo_path(path: &str) -> String {
    path.replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(target_os = "linux")]
fn resolve_cgroup_dir(mount: &CgroupMount, cgroup_path: &str) -> Option<PathBuf> {
    let cgroup_path = Path::new(cgroup_path);
    let relative = cgroup_path
        .strip_prefix(&mount.root)
        .unwrap_or_else(|_| cgroup_path.strip_prefix("/").unwrap_or(cgroup_path));
    let mut resolved = mount.mount_point.clone();
    for component in relative.components() {
        match component {
            Component::Normal(component) => resolved.push(component),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(resolved)
}

#[cfg(target_os = "linux")]
fn cgroup_hierarchy_available(
    current_dir: &Path,
    mount_point: &Path,
    limit_name: &str,
    usage_name: &str,
) -> Option<u64> {
    if !current_dir.starts_with(mount_point) {
        return None;
    }

    let mut minimum = None;
    let mut directory = Some(current_dir);
    while let Some(current) = directory {
        let limit_path = current.join(limit_name);
        let usage_path = current.join(usage_name);
        if let Some(available) = cgroup_available_from_paths(&limit_path, &usage_path) {
            minimum = Some(minimum.map_or(available, |value: u64| value.min(available)));
        }
        if current == mount_point {
            break;
        }
        directory = current.parent();
    }
    minimum
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn platform_available_memory_bytes() -> Option<u64> {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        return None;
    }

    let mut count = libc::HOST_VM_INFO64_COUNT;
    let mut stats = unsafe { std::mem::zeroed::<libc::vm_statistics64>() };
    let result = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            &mut stats as *mut libc::vm_statistics64 as *mut _,
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        return None;
    }

    let available_pages = u64::from(stats.free_count)
        .saturating_add(u64::from(stats.inactive_count))
        .saturating_add(u64::from(stats.purgeable_count))
        .saturating_add(u64::from(stats.speculative_count));
    Some(available_pages.saturating_mul(page_size as u64))
}

#[cfg(target_os = "windows")]
fn platform_available_memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = unsafe { std::mem::zeroed::<MEMORYSTATUSEX>() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        None
    } else {
        Some(status.ullAvailPhys)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_available_memory_bytes() -> Option<u64> {
    None
}

#[cfg(any(test, target_os = "linux"))]
fn parse_meminfo_bytes(contents: &str, key: &str) -> Option<u64> {
    contents.lines().find_map(|line| {
        line.strip_prefix(key)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .and_then(|kb| kb.checked_mul(1024))
    })
}

#[cfg(any(test, target_os = "linux"))]
fn parse_cgroup_limit(value: &str) -> Option<u64> {
    let value = value.trim();
    if value == "max" {
        None
    } else {
        value.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_available_memory() {
        let meminfo = "MemTotal:       8388608 kB\nMemAvailable:   3145728 kB\n";
        assert_eq!(
            parse_meminfo_bytes(meminfo, "MemAvailable:"),
            Some(3 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn cgroup_max_is_not_treated_as_a_numeric_limit() {
        assert_eq!(parse_cgroup_limit("max\n"), None);
        assert_eq!(parse_cgroup_limit("2147483648\n"), Some(2_147_483_648));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_current_v2_and_v1_cgroups() {
        let cgroups = "0::/user.slice/session.scope\n5:memory,cpu:/legacy.scope\n";
        assert_eq!(
            parse_cgroup_path(cgroups, None).as_deref(),
            Some("/user.slice/session.scope")
        );
        assert_eq!(
            parse_cgroup_path(cgroups, Some("memory")).as_deref(),
            Some("/legacy.scope")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_namespaced_cgroup_mounts_without_path_escape() {
        let mount = CgroupMount {
            root: PathBuf::from("/user.slice"),
            mount_point: PathBuf::from("/sys/fs/cgroup"),
        };
        assert_eq!(
            resolve_cgroup_dir(&mount, "/user.slice/session.scope"),
            Some(PathBuf::from("/sys/fs/cgroup/session.scope"))
        );
        assert_eq!(resolve_cgroup_dir(&mount, "/../escape"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_cgroup_mountinfo() {
        let mountinfo = concat!(
            "496 495 0:30 / /sys/fs/cgroup rw - cgroup2 cgroup rw,memory_recursiveprot\n",
            "497 495 0:31 / /sys/fs/cgroup/memory rw - cgroup cgroup rw,memory\n",
        );
        assert_eq!(
            parse_cgroup_mount(mountinfo, "cgroup2", None),
            Some(CgroupMount {
                root: PathBuf::from("/"),
                mount_point: PathBuf::from("/sys/fs/cgroup"),
            })
        );
        assert_eq!(
            parse_cgroup_mount(mountinfo, "cgroup", Some("memory")),
            Some(CgroupMount {
                root: PathBuf::from("/"),
                mount_point: PathBuf::from("/sys/fs/cgroup/memory"),
            })
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn applies_the_tightest_cgroup_ancestor_limit() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let child = root.join("child");
        fs::create_dir(&child).unwrap();
        fs::write(root.join("memory.max"), "1073741824\n").unwrap();
        fs::write(root.join("memory.current"), "268435456\n").unwrap();
        fs::write(child.join("memory.max"), "max\n").unwrap();
        fs::write(child.join("memory.current"), "134217728\n").unwrap();

        assert_eq!(
            cgroup_hierarchy_available(&child, root, "memory.max", "memory.current"),
            Some(768 * 1024 * 1024)
        );
    }
}
