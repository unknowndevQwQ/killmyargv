use std::ffi::c_char;

#[cfg(feature = "clobber_environ")]
use log::{debug, trace};

// environ() copied from https://github.com/rust-lang/rust/blob/1.84.0/library/std/src/sys/pal/unix/os.rs#L581-L616
// Use `_NSGetEnviron` on Apple platforms.
//
// `_NSGetEnviron` is the documented alternative (see `man environ`), and has
// been available since the first versions of both macOS and iOS.
//
// Nowadays, specifically since macOS 10.8, `environ` has been exposed through
// `libdyld.dylib`, which is linked via. `libSystem.dylib`:
// <https://github.com/apple-oss-distributions/dyld/blob/dyld-1160.6/libdyld/libdyldGlue.cpp#L913>
//
// So in the end, it likely doesn't really matter which option we use, but the
// performance cost of using `_NSGetEnviron` is extremely miniscule, and it
// might be ever so slightly more supported, so let's just use that.
//
// NOTE: The header where this is defined (`crt_externs.h`) was added to the
// iOS 13.0 SDK, which has been the source of a great deal of confusion in the
// past about the availability of this API.
//
// NOTE(madsmtm): Neither this nor using `environ` has been verified to not
// cause App Store rejections; if this is found to be the case, an alternative
// implementation of this is possible using `[NSProcessInfo environment]`
// - which internally uses `_NSGetEnviron` and a system-wide lock on the
// environment variables to protect against `setenv`, so using that might be
// desirable anyhow? Though it also means that we have to link to Foundation.
#[cfg(target_vendor = "apple")]
pub unsafe fn environ() -> *mut *const *const c_char {
    libc::_NSGetEnviron() as *mut *const *const c_char
}

// Use the `environ` static which is part of POSIX.
#[cfg(not(target_vendor = "apple"))]
pub unsafe fn environ() -> *mut *const *const c_char {
    extern "C" {
        static mut environ: *const *const c_char;
    }
    &raw mut environ
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
// copied from https://github.com/rust-lang/rust/blob/1.84.0/library/src/std/sys/pal/unix/os.rs.html#L626-L658
pub(super) fn addr() -> Option<(usize, *const *const c_char)> {
    unsafe {
        let envp = *environ();
        let mut environ = envp;
        // I often forget: Where did the number of elements go?
        debug!("environ={environ:?}, point to: {:?}", *environ);
        if !environ.is_null() && !(*environ).is_null() {
            let mut element = 0;
            debug!("begin: environ[{element}]={:?}, ptr={environ:?}", *environ);
            while !(*environ).is_null() {
                trace!(
                    "current: environ[{element}]={:?}, ptr={environ:?}",
                    *environ
                );
                environ = environ.add(1);
                element += 1;
                if (*environ).is_null() {
                    debug!("end: environ[{element}]={:?}, ptr={environ:?}", *environ);
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
