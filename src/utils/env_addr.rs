#[cfg_attr(
    not(any(feature = "clobber_environ", feature = "stack_walking")),
    allow(unused_imports)
)]
use std::{
    ffi::{c_char, CStr, CString},
    ptr,
};

use super::MemInfo;

use log::{trace, warn};

// environ() copied from https://doc.rust-lang.org/src/std/sys/unix/os.rs.html#491-502
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
pub(super) fn addr() -> Option<MemInfo> {
    None
}

#[cfg(feature = "clobber_environ")]
// copied from https://doc.rust-lang.org/src/std/sys/unix/os.rs.html#512-544
pub(super) fn addr() -> Option<MemInfo> {
    unsafe {
        let envp = *environ();
        let mut environ = envp;
        trace!("environ ptr: {environ:?} {:?}", *environ); // I often forget: Where did the number of elements go?
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

            let mut byte_len = 0;
            let mut end_addr = *envp;
            let mut copy: Vec<CString> = Vec::with_capacity(element);
            for i in 0..element {
                let val_ptr = *environ.add(i);
                let val_len = CStr::from_ptr(val_ptr).to_bytes_with_nul().len();
                copy.push(CStr::from_ptr(val_ptr).into());
                if i < element {
                    // Decide elsewhere whether to exclude nul.
                    byte_len += val_len;

                    trace!("env collect: recorded len={byte_len}, ptr={val_ptr:?}, len={val_len}, next ptr={:?}",
                        val_ptr.add(val_len - 1)
                    );
                    if i + 1 == element {
                        // It is assumed that environ must never have an element
                        // of length 0, otherwise unpredictable results would occur.
                        // Perhaps it would be better to add 1 byte manually when
                        // calculating the length.
                        // Avoid overstepping the bounds.
                        end_addr = val_ptr.add(val_len - 1);
                    }
                }
            }
            trace!(
                "envc: {element}, env_ptr: {envp:?}, addr: {:?} -> {end_addr:?}, len: {byte_len}",
                *envp
            );
            if byte_len != 0 {
                Some(MemInfo {
                    begin_addr: *envp,
                    end_addr,
                    byte_len,
                    element,
                    copy,
                    pointer_addr: envp,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
