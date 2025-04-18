use super::EnvError;
use crate::env_addr::envptr;
use std::ffi::{c_char, c_int};

use log::{debug, trace};

// init() copied from https://doc.rust-lang.org/src/std/sys/unix/args.rs.html#12-15
// But what's the point?
#[allow(unused)]
/// One-time global initialization.
/// The above statement was made by rust std, and no one here is responsible for that statement.
pub unsafe fn init(argc: isize, argv: *const *const u8) {
    imp::init(argc, argv)
}

/// stack walking:
/// https://github.com/rust-lang/rust/pull/66547#issuecomment-556013952
/// and https://github.com/leo60228/libargs
/// some links:
/// https://github.com/rust-lang/rust/issues/105999
/// https://github.com/rust-lang/rust/pull/106001
/// https://github.com/rust-lang/rust/commit/e97203c3f893893611818997bbeb0116ded2605f
pub(super) fn addr() -> Result<(usize, *const *const c_char), EnvError> {
    let (argc, argv) = imp::argc_argv();
    debug!("imp argc={argc}, argv={argv:?}, is null={}", argv.is_null());

    if cfg!(any(
        feature = "compute_argv",
        feature = "stack_walking",
        feature = "force_walking"
    )) {
        if argv.is_null() || (unsafe { *argv }).is_null() {
            debug!("failed from imp get argv, try compute/stackwalking");
            comp_argv()
        } else {
            Ok((argc as usize, argv))
        }
    } else {
        if argv.is_null() || (unsafe { *argv }).is_null() {
            Err(EnvError::InvalidArgvPointer)
        } else {
            Ok((argc as usize, argv))
        }
    }
}

// from: https://github.com/leo60228/libargs/blob/master/src/lib.rs#L16-L30
// or `fn from_backtrace(...) -> (...)`?
fn from_stack_walking(environ: *const *const c_char) -> (usize, *const *const c_char) {
    let mut walk_environ = environ as *const usize;
    walk_environ = walk_environ.wrapping_sub(1);
    let mut i = 0;

    loop {
        let argc_ptr = walk_environ.wrapping_sub(1) as *const c_int;
        let argc = unsafe { *argc_ptr };
        if argc == i {
            break (argc as usize, walk_environ as *const *const c_char);
        }
        walk_environ = walk_environ.wrapping_sub(1);
        i += 1;
    }
}

fn comp_argv() -> Result<(usize, *const *const i8), EnvError> {
    let envp = unsafe { envptr().ok_or(EnvError::FailedToGetArgvPointer) }?;

    if cfg!(feature = "force_walking") {
        debug!("forge walking...");
        return Ok(from_stack_walking(envp));
    } else {
        use std::{
            env::args_os,
            ffi::{CStr, OsStr},
            os::unix::ffi::OsStrExt,
        };
        let mut args = args_os();
        trace!("std args: {:#?}", &args);
        if args.len() == 0 {
            debug!("std args is empty, try stack walking...");
            if cfg!(feature = "stack_walking") {
                return Ok(from_stack_walking(envp));
            } else {
                return Err(EnvError::FailedToGetArgvPointer);
            }
        }

        let std_argc = args.len();
        // *environ[] == *argv[] + argc + 1, aka
        // *argv[] = *environ[] - (argc + 1)
        unsafe {
            let comp_argv = envp.sub(std_argc + 1);
            trace!("environ={envp:?}, std argc={std_argc:?}, computed argv={comp_argv:?}, point to: {:?}", (*comp_argv));
            if comp_argv.is_null() || (*comp_argv).is_null() {
                return Err(EnvError::InvalidArgvPointer);
            }

            let frist = args.next().ok_or(EnvError::InvalidArgvPointer)?;
            trace!("try read computed argv[0]");
            let argv_frist = OsStr::from_bytes(CStr::from_ptr(*comp_argv).to_bytes());
            trace!("computed argv[0]={argv_frist:?}, std argv[0]={frist:?}");
            if argv_frist == frist {
                Ok((std_argc, comp_argv))
            } else {
                Err(EnvError::InvalidArgvPointer)
            }
        }
    }
}

// imp::argc_argv() copied from: https://github.com/rust-lang/rust/blob/1.84.1/library/std/src/sys/pal/unix/args.rs#L96-L182
#[rustfmt::skip]
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "emscripten",
    target_os = "haiku",
    target_os = "l4re",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vxworks",
    target_os = "horizon",
    target_os = "aix",
    target_os = "nto",
    target_os = "hurd",
    target_os = "rtems",
    target_os = "nuttx",
))]
mod imp {
    use core::ffi::c_char;
    use core::ptr;
    use core::sync::atomic::{AtomicIsize, AtomicPtr, Ordering};

    // The system-provided argc and argv, which we store in static memory
    // here so that we can defer the work of parsing them until its actually
    // needed.
    //
    // Note that we never mutate argv/argc, the argv array, or the argv
    // strings, which allows the code in this file to be very simple.
    static ARGC: AtomicIsize = AtomicIsize::new(0);
    static ARGV: AtomicPtr<*const u8> = AtomicPtr::new(ptr::null_mut());

    unsafe fn really_init(argc: isize, argv: *const *const u8) {
        // These don't need to be ordered with each other or other stores,
        // because they only hold the unmodified system-provide argv/argc.
        ARGC.store(argc, Ordering::Relaxed);
        ARGV.store(argv as *mut _, Ordering::Relaxed);
    }

    #[inline(always)]
    pub unsafe fn init(argc: isize, argv: *const *const u8) {
        // on GNU/Linux if we are main then we will init argv and argc twice, it "duplicates work"
        // BUT edge-cases are real: only using .init_array can break most emulators, dlopen, etc.
        really_init(argc, argv);
    }

    /// glibc passes argc, argv, and envp to functions in .init_array, as a non-standard extension.
    /// This allows `std::env::args` to work even in a `cdylib`, as it does on macOS and Windows.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[used]
    #[link_section = ".init_array.00099"]
    static ARGV_INIT_ARRAY: extern "C" fn(
        core::ffi::c_int,
        *const *const u8,
        *const *const u8,
    ) = {
        extern "C" fn init_wrapper(
            argc: core::ffi::c_int,
            argv: *const *const u8,
            _envp: *const *const u8,
        ) {
            unsafe {
                really_init(argc as isize, argv);
            }
        }
        init_wrapper
    };

    pub fn argc_argv() -> (isize, *const *const c_char) {
        // Load ARGC and ARGV, which hold the unmodified system-provided
        // argc/argv, so we can read the pointed-to memory without atomics or
        // synchronization.
        //
        // If either ARGC or ARGV is still zero or null, then either there
        // really are no arguments, or someone is asking for `args()` before
        // initialization has completed, and we return an empty list.
        let argv = ARGV.load(Ordering::Relaxed);
        let argc = if argv.is_null() { 0 } else { ARGC.load(Ordering::Relaxed) };

        // Cast from `*mut *const u8` to `*const *const c_char`
        (argc, argv.cast())
    }
}

#[rustfmt::skip]
// Use `_NSGetArgc` and `_NSGetArgv` on Apple platforms.
//
// Even though these have underscores in their names, they've been available
// since the first versions of both macOS and iOS, and are declared in
// the header `crt_externs.h`.
//
// NOTE: This header was added to the iOS 13.0 SDK, which has been the source
// of a great deal of confusion in the past about the availability of these
// APIs.
//
// NOTE(madsmtm): This has not strictly been verified to not cause App Store
// rejections; if this is found to be the case, the previous implementation
// of this used `[[NSProcessInfo processInfo] arguments]`.
#[cfg(target_vendor = "apple")]
mod imp {
    use core::ffi::{c_char, c_int};

    pub unsafe fn init(_argc: isize, _argv: *const *const u8) {
        // No need to initialize anything in here, `libdyld.dylib` has already
        // done the work for us.
    }

    pub fn argc_argv() -> (isize, *const *const c_char) {
        extern "C" {
            // These functions are in crt_externs.h.
            fn _NSGetArgc() -> *mut c_int;
            fn _NSGetArgv() -> *mut *mut *mut c_char;
        }

        // SAFETY: The returned pointer points to a static initialized early
        // in the program lifetime by `libdyld.dylib`, and as such is always
        // valid.
        //
        // NOTE: Similar to `_NSGetEnviron`, there technically isn't anything
        // protecting us against concurrent modifications to this, and there
        // doesn't exist a lock that we can take. Instead, it is generally
        // expected that it's only modified in `main` / before other code
        // runs, so reading this here should be fine.
        let argc = unsafe { _NSGetArgc().read() };
        // SAFETY: Same as above.
        let argv = unsafe { _NSGetArgv().read() };

        // Cast from `*mut *mut c_char` to `*const *const c_char`
        (argc as isize, argv.cast())
    }
}
