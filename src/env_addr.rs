use std::{ffi::c_char, ptr};

#[cfg(feature = "clobber_environ")]
use log::trace;

// environ() copied from https://github.com/rust-lang/rust/blob/1.68.0/library/src/std/sys/unix/os.rs.html#491-502
#[cfg(target_os = "macos")]
unsafe fn environ() -> *mut *const *const c_char {
    libc::_NSGetEnviron() as *mut *const *const c_char
}

#[cfg(not(target_os = "macos"))]
unsafe fn environ() -> *mut *const *const c_char {
    extern "C" {
        static mut environ: *const *const c_char;
    }
    ptr::addr_of_mut!(environ)
}

// The main reason for envptr() to be pub is that it needs to be used to derive the argv pointer.
#[allow(unused)]
pub(super) unsafe fn envptr() -> Option<*const *const c_char> {
    let envp = *environ();
    if envp.is_null() {
        None
    } else {
        Some(envp)
    }
}

// environ is not used by default.
#[cfg(not(feature = "clobber_environ"))]
pub(super) fn addr() -> Option<(usize, *const *const c_char)> {
    None
}

#[cfg(feature = "clobber_environ")]
// copied from https://github.com/rust-lang/rust/blob/1.68.0/library/src/std/sys/unix/os.rs.html#512-544
pub(super) fn addr() -> Option<(usize, *const *const c_char)> {
    unsafe {
        let envp = *environ();
        let mut environ = envp;
        // I often forget: Where did the number of elements go?
        trace!("environ ptr: {environ:?} {:?}", *environ);
        if !environ.is_null() && !(*environ).is_null() {
            let mut element = 0;
            trace!(
                "frist env: element: {element}, ptr: {environ:?} {:?}",
                *environ
            );
            while !(*environ).is_null() {
                trace!(
                    "currter env: element: {element}, ptr: {environ:?} {:?}",
                    *environ
                );
                environ = environ.add(1);
                element += 1;
                if (*environ).is_null() {
                    trace!(
                        "end env: element: {element}, ptr: {environ:?} {:?}",
                        *environ
                    );
                    environ = envp;
                    break;
                }
            }
            Some((element as usize, environ))
        } else {
            None
        }
    }
}
