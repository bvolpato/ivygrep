//! Which allocator serves this process.
//!
//! The 64-bit musl builds use jemalloc (`src/main.rs`). jemalloc fixes its
//! page size when it is built, and a build for smaller pages than the kernel
//! uses aborts at startup. Emulation cannot catch that: QEMU user mode runs
//! with the host's 4 KiB pages. So `ig hardware` reports what the binary was
//! built with, and the release and E2E workflows assert it.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllocatorReport {
    /// `jemalloc` or `system`.
    pub name: String,
    /// Page size that jemalloc was built for, in bytes. `None` for the system
    /// allocator.
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
    if let Some(page_size) = jemalloc::page_size_if_global() {
        return AllocatorReport {
            name: "jemalloc".to_string(),
            page_size: Some(page_size),
        };
    }
    AllocatorReport {
        name: "system".to_string(),
        page_size: None,
    }
}

#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
mod jemalloc {
    use std::ffi::{CStr, c_uint, c_void};

    /// jemalloc's page size, when jemalloc is this process's global allocator.
    /// jemalloc is linked into every musl artifact of this crate, but only a
    /// binary that declares it as `#[global_allocator]` allocates through it.
    /// So the probe asks jemalloc whether it owns a fresh Rust allocation:
    /// `arenas.lookup` fails for memory that came from another allocator.
    pub(super) fn page_size_if_global() -> Option<u64> {
        let probe = std::hint::black_box(Box::new(0u64));
        let address: *const c_void = std::ptr::from_ref::<u64>(&probe).cast();
        read::<c_uint>(c"arenas.lookup", Some(address))?;
        read::<usize>(c"arenas.page", None).map(|page_size| page_size as u64)
    }

    /// Read one `mallctl` value, passing `input` as the new value when the
    /// name takes one.
    fn read<T: Copy + Default>(name: &CStr, input: Option<*const c_void>) -> Option<T> {
        let mut value = T::default();
        let mut value_len = std::mem::size_of::<T>();
        let mut input = input;
        let (new_value, new_len) = match input.as_mut() {
            Some(address) => (
                std::ptr::from_mut(address).cast::<c_void>(),
                std::mem::size_of::<*const c_void>(),
            ),
            None => (std::ptr::null_mut(), 0),
        };
        // SAFETY: `name` is NUL-terminated, `value` and `value_len` describe
        // one writable `T`, and `new_value` is null or points at one pointer
        // that lives across the call. jemalloc only looks the address up in
        // its extent map; it never dereferences it.
        let status = unsafe {
            tikv_jemalloc_sys::mallctl(
                name.as_ptr(),
                std::ptr::from_mut(&mut value).cast::<c_void>(),
                &mut value_len,
                new_value,
                new_len,
            )
        };
        (status == 0 && value_len == std::mem::size_of::<T>()).then_some(value)
    }
}
