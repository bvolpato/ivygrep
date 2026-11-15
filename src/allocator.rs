//! Which allocator serves this process.
//!
//! The 64-bit musl builds use jemalloc (`src/main.rs`). jemalloc fixes its
//! page size when it is built, and a build for smaller pages than the kernel
//! uses aborts at startup. Emulation cannot catch that: QEMU user mode runs
//! with the host's 4 KiB pages. So `ig hardware` reports what the binary was
//! built with, and the release and E2E workflows assert it.

use serde::Serialize;

#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
static JEMALLOC_IS_GLOBAL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Record the allocator selected by the binary before it starts serving requests.
/// Library callers retain the system allocator report unless they register their
/// own `#[global_allocator]` jemalloc here.
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
pub fn register_jemalloc() {
    JEMALLOC_IS_GLOBAL.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllocatorReport {
    /// `jemalloc` or `system`.
    pub name: String,
    /// Page size that jemalloc was built for, in bytes, when available.
    pub page_size: Option<u64>,
}

impl AllocatorReport {
    pub fn label(&self) -> String {
        match self.page_size {
            Some(page_size) => format!("{} ({page_size}-byte pages)", self.name),
            None => self.name.clone(),
        }
    }
}

pub fn inspect() -> AllocatorReport {
    #[cfg(all(target_env = "musl", target_pointer_width = "64"))]
    if JEMALLOC_IS_GLOBAL.load(std::sync::atomic::Ordering::Relaxed) {
        return AllocatorReport {
            name: "jemalloc".to_string(),
            page_size: jemalloc::page_size(),
        };
    }
    AllocatorReport {
        name: "system".to_string(),
        page_size: None,
    }
}

#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
mod jemalloc {
    use std::ffi::c_void;

    pub(super) fn page_size() -> Option<u64> {
        let mut value = 0usize;
        let mut value_len = std::mem::size_of::<usize>();
        // SAFETY: `arenas.page` reads a size_t into the valid output buffer.
        // The name is NUL-terminated and no input value is supplied.
        let status = unsafe {
            tikv_jemalloc_sys::mallctl(
                c"arenas.page".as_ptr(),
                std::ptr::from_mut(&mut value).cast::<c_void>(),
                &mut value_len,
                std::ptr::null_mut(),
                0,
            )
        };
        (status == 0 && value_len == std::mem::size_of::<usize>()).then_some(value as u64)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_reports_system_allocator_without_registration() {
        assert_eq!(
            super::inspect(),
            super::AllocatorReport {
                name: "system".to_string(),
                page_size: None,
            }
        );
    }
}
