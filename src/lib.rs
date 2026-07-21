//! Rust bindings for [mimalloc](https://github.com/microsoft/mimalloc) v3.4.3 —
//! a compact general purpose allocator with excellent performance by Microsoft Research.
//!
//! # Usage
//!
//! Set `MiMalloc` as the global allocator in your application:
//!
//! ```rust
//! use mimalloc::MiMalloc;
//!
//! #[global_allocator]
//! static GLOBAL: MiMalloc = MiMalloc;
//! ```

use std::alloc::{GlobalAlloc, Layout};
use std::ffi::c_void;

unsafe extern "C" {
    fn mi_malloc(size: usize) -> *mut c_void;
    fn mi_calloc(count: usize, size: usize) -> *mut c_void;
    fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
    fn mi_free(p: *mut c_void);

    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    fn mi_realloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;

    fn mi_usable_size(p: *const c_void) -> usize;
}

/// The mimalloc allocator.
///
/// Use as a `#[global_allocator]` to replace the default Rust allocator:
///
/// ```rust
/// use mimalloc::MiMalloc;
///
/// #[global_allocator]
/// static GLOBAL: MiMalloc = MiMalloc;
/// ```
pub struct MiMalloc;

unsafe impl GlobalAlloc for MiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            unsafe { mi_malloc(layout.size()) as *mut u8 }
        } else {
            unsafe { mi_malloc_aligned(layout.size(), layout.align()) as *mut u8 }
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { mi_free(ptr as *mut c_void) };
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            unsafe { mi_calloc(1, layout.size()) as *mut u8 }
        } else {
            unsafe { mi_zalloc_aligned(layout.size(), layout.align()) as *mut u8 }
        }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= new_size {
            unsafe { mi_realloc(ptr as *mut c_void, new_size) as *mut u8 }
        } else {
            unsafe { mi_realloc_aligned(ptr as *mut c_void, new_size, layout.align()) as *mut u8 }
        }
    }
}

unsafe impl Send for MiMalloc {}
unsafe impl Sync for MiMalloc {}

/// Minimum alignment guaranteed by the platform's default allocator.
/// mimalloc guarantees at least this alignment for non-aligned calls.
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "sparc64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
))]
const MIN_ALIGN: usize = 16;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "mips64",
    target_arch = "s390x",
    target_arch = "sparc64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
)))]
const MIN_ALIGN: usize = 8;

/// Returns the usable size of an allocated memory block.
///
/// The returned size can be larger than the originally requested size.
/// Returns 0 if `ptr` is null.
#[inline]
pub fn usable_size(ptr: *const u8) -> usize {
    unsafe { mi_usable_size(ptr as *const c_void) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Layout;

    #[global_allocator]
    static ALLOC: MiMalloc = MiMalloc;

    #[test]
    fn alloc_and_dealloc() {
        unsafe {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = ALLOC.alloc(layout);
            assert!(!ptr.is_null());
            ALLOC.dealloc(ptr, layout);
        }
    }

    #[test]
    fn alloc_zeroed_is_zero() {
        unsafe {
            let layout = Layout::from_size_align(128, 8).unwrap();
            let ptr = ALLOC.alloc_zeroed(layout);
            assert!(!ptr.is_null());
            let slice = std::slice::from_raw_parts(ptr, 128);
            assert!(slice.iter().all(|&b| b == 0));
            ALLOC.dealloc(ptr, layout);
        }
    }

    #[test]
    fn realloc_works() {
        unsafe {
            let layout = Layout::from_size_align(32, 8).unwrap();
            let ptr = ALLOC.alloc(layout);
            assert!(!ptr.is_null());
            let new_ptr = ALLOC.realloc(ptr, layout, 256);
            assert!(!new_ptr.is_null());
            ALLOC.dealloc(new_ptr, Layout::from_size_align(256, 8).unwrap());
        }
    }

    #[test]
    fn aligned_alloc() {
        unsafe {
            let layout = Layout::from_size_align(64, 256).unwrap();
            let ptr = ALLOC.alloc(layout);
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % 256, 0, "pointer should be 256-byte aligned");
            ALLOC.dealloc(ptr, layout);
        }
    }

    #[test]
    fn aligned_alloc_zeroed() {
        unsafe {
            let layout = Layout::from_size_align(64, 256).unwrap();
            let ptr = ALLOC.alloc_zeroed(layout);
            assert!(!ptr.is_null());
            assert_eq!(ptr as usize % 256, 0, "pointer should be 256-byte aligned");
            let slice = std::slice::from_raw_parts(ptr, 64);
            assert!(slice.iter().all(|&b| b == 0));
            ALLOC.dealloc(ptr, layout);
        }
    }

    #[test]
    fn usable_size_works() {
        unsafe {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = ALLOC.alloc(layout);
            assert!(!ptr.is_null());
            let size = usable_size(ptr);
            assert!(size >= 64);
            ALLOC.dealloc(ptr, layout);
        }
    }

    #[test]
    fn vec_uses_mimalloc() {
        let mut v: Vec<u32> = Vec::new();
        for i in 0..1000 {
            v.push(i);
        }
        assert_eq!(v.len(), 1000);
        assert_eq!(v[999], 999);
    }

    #[test]
    fn box_uses_mimalloc() {
        let b = Box::new(42u64);
        assert_eq!(*b, 42);
    }
}
