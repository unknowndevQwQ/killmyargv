use super::{Error, MemInfo};
use log::{trace, warn};
// init() copied from https://doc.rust-lang.org/src/std/sys/unix/args.rs.html#12-15
// But what's the point?
#[allow(unused)]
/// One-time global initialization.
/// The above statement was made by rust std, and no one here is responsible for that statement.
pub unsafe fn init(argc: isize, argv: *const *const std::ffi::c_char) {
    imp::init(argc, argv)
}

///
/// stack walking:
/// https://github.com/rust-lang/rust/pull/66547#issuecomment-556013952
/// and https://github.com/leo60228/libargs
/// some links:
/// https://github.com/rust-lang/rust/issues/105999
/// https://github.com/rust-lang/rust/pull/106001
/// https://github.com/rust-lang/rust/commit/e97203c3f893893611818997bbeb0116ded2605f
pub(super) fn addr() -> Result<MemInfo, Error> {
    // todo: as an alternative, perform a walking stack to get the argv pointer.
    imp::addr()
}

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
    target_os = "horizon"
))]
mod imp {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    use std::ffi::c_int;
    use std::{
        ffi::{c_char, CStr, CString, OsStr},
        os::unix::ffi::OsStrExt,
        ptr,
        sync::atomic::{AtomicIsize, AtomicPtr, Ordering},
    }; // Required for linux-gnu only, to avoid warnings.

    use super::{Error, Error::InvalidArgvPointerError, MemInfo};

    use log::{trace, warn};

    // The system-provided argc and argv, which we store in static memory
    // here so that we can defer the work of parsing them until its actually
    // needed.
    //
    // Note that we never mutate argv/argc, the argv array, or the argv
    // strings, which allows the code in this file to be very simple.
    static ARGC: AtomicIsize = AtomicIsize::new(0);
    static ARGV: AtomicPtr<*const c_char> = AtomicPtr::new(ptr::null_mut());

    unsafe fn really_init(argc: isize, argv: *const *const c_char) {
        // These don't need to be ordered with each other or other stores,
        // because they only hold the unmodified system-provide argv/argc.
        ARGC.store(argc, Ordering::Relaxed);
        ARGV.store(argv as *mut _, Ordering::Relaxed);
    }

    #[cfg_attr(all(target_os = "linux", target_env = "gnu"), allow(unused))]
    #[inline(always)]
    pub unsafe fn init(_argc: isize, _argv: *const *const c_char) {
        // On Linux-GNU, we rely on `ARGV_INIT_ARRAY` below to initialize
        // `ARGC` and `ARGV`. But in Miri that does not actually happen so we
        // still initialize here.
        #[cfg(any(miri, not(all(target_os = "linux", target_env = "gnu"))))]
        really_init(_argc, _argv);
    }

    /// glibc passes argc, argv, and envp to functions in .init_array, as a non-standard extension.
    /// This allows `std::env::args` to work even in a `cdylib`, as it does on macOS and Windows.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[used]
    #[link_section = ".init_array.00099"]
    static ARGV_INIT_ARRAY: extern "C" fn(c_int, *const *const c_char, *const *const c_char) = {
        extern "C" fn init_wrapper(
            argc: c_int,
            argv: *const *const c_char,
            _envp: *const *const c_char,
        ) {
            unsafe {
                really_init(argc as isize, argv);
            }
        }
        init_wrapper
    };

    pub(super) fn addr() -> Result<MemInfo, Error> {
        unsafe {
            // Load ARGC and ARGV, which hold the unmodified system-provided
            // argc/argv, so we can read the pointed-to memory without atomics
            // or synchronization.
            //
            // If either ARGC or ARGV is still zero or null, then either there
            // really are no arguments, or someone is asking for `args()`
            // before initialization has completed, and we return an empty
            // list.

            // dirty
            let mut argv = ARGV.load(Ordering::Relaxed);
            let mut argc = ARGC.load(Ordering::Relaxed);
            trace!("argc: {argc}, argv: {argv:?}");

            #[cfg(any(feature = "comp_argv", feature = "stack_walking"))]
            if argv.is_null() || (*argv).is_null() {
                use crate::utils::env_addr::envptr;
                use std::env::args_os;

                let mut args = args_os();
                let mut args_is_empty = false;
                //
                trace!("args: {:#?}", &args);
                if args.len() == 0 {
                    args_is_empty = true;
                    trace!("std args is empty");
                }
                match envptr() {
                    Some(envp) => {
                        if args_is_empty == false {
                            let std_argc = args.len();
                            // *environ[] == *argv[] + argc + 1, aka
                            // *argv[] = *environ[] - (argc + 1)
                            let comp_argv = envp.sub(std_argc + 1);
                            trace!("environ ptr: {envp:?}, argc from std: {std_argc:?}, computed argv: {comp_argv:?}");
                            if comp_argv.is_null() || (*comp_argv).is_null() {
                                return Err(InvalidArgvPointerError());
                            }

                            if let Some(frist) = args.next() {
                                let argv_frist =
                                    OsStr::from_bytes(CStr::from_ptr(*comp_argv).to_bytes());
                                dbg!(argv_frist, &frist);
                                if argv_frist == frist {
                                    argv = comp_argv as *mut *const c_char;
                                    argc = args.len() as isize;
                                }
                            }
                            //
                        }
                    }
                    None => return Err(InvalidArgvPointerError()),
                }
            }
            #[cfg(not(feature = "stack_walking"))]
            if argv.is_null() || (*argv).is_null() {
                return Err(InvalidArgvPointerError());
            }

            let mut byte_len = 0;
            let mut end_addr = *argv;
            let mut copy: Vec<CString> = Vec::with_capacity(argc as usize);
            for i in 0..argc {
                let val_ptr = *argv.offset(i);
                let val_len = CStr::from_ptr(val_ptr).to_bytes_with_nul().len();
                copy.push(CStr::from_ptr(val_ptr).into());
                if i < argc {
                    // Decide elsewhere whether to exclude nul.
                    byte_len += val_len;
                    trace!("argv collect: recorded len={byte_len}, ptr={val_ptr:?}, len={val_len}, next ptr={:?}",
                        val_ptr.offset((val_len - 1) as isize)
                    );
                    if i + 1 == argc {
                        // Perhaps it would be better to add 1 byte manually
                        // when calculating the length.
                        // Avoid overstepping the bounds.
                        end_addr = val_ptr.offset((val_len - 1) as isize);
                    }
                }
            }
            trace!(
                "argc: {argc}, argv_ptr: {argv:?}, addr: {:?} -> {end_addr:?}, len: {byte_len}",
                *argv
            );
            if byte_len != 0 {
                Ok(MemInfo {
                    begin_addr: *argv,
                    end_addr,
                    byte_len,
                    element: argc as usize,
                    copy,
                    pointer_addr: argv,
                })
            } else {
                Err(InvalidArgvPointerError())
            }
        }
    }
}

// Not yet tested
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "watchos"))]
mod imp {
    use std::ffi::{c_char, c_int, CStr, CString};

    use super::{Error, Error::InvalidArgvPointerError, MemInfo};

    pub unsafe fn init(_argc: isize, _argv: *const *const c_char) {}

    #[cfg(target_os = "macos")]
    pub fn addr() -> Result<MemInfo, Error> {
        extern "C" {
            // These functions are in crt_externs.h.
            fn _NSGetArgc() -> *mut c_int;
            fn _NSGetArgv() -> *mut *mut *mut c_char;
        }

        unsafe {
            let (argc, argv) = (
                *_NSGetArgc() as isize,
                *_NSGetArgv() as *const *const c_char,
            );
            let mut byte_len = 0;
            let mut end_addr = *argv;
            let mut copy: Vec<CString> = Vec::with_capacity(argc as usize);
            for i in 0..argc {
                let val_ptr = *argv.offset(i);
                let val_len = CStr::from_ptr(val_ptr).to_bytes_with_nul().len();
                copy.push(CStr::from_ptr(val_ptr).into());
                if i < argc {
                    // Decide elsewhere whether to exclude nul.
                    byte_len += val_len;
                    trace!("argv collect: recorded len={byte_len}, ptr={val_ptr:?}, len={val_len}, next ptr={:?}",
                        val_ptr.offset((val_len - 1) as isize)
                    );
                    if i + 1 == argc {
                        // Perhaps it would be better to add 1 byte manually when calculating the length.
                        // Avoid overstepping the bounds.
                        end_addr = val_ptr.offset((val_len - 1) as isize);
                    }
                }
            }
            if end_addr != *argv {
                Ok(MemInfo {
                    begin_addr: *argv,
                    end_addr,
                    byte_len,
                    element: argc as usize,
                    copy,
                    pointer_addr: argv,
                })
            } else {
                Error(InvalidArgvPointerError())
            }
        };
    }

    // As _NSGetArgc and _NSGetArgv aren't mentioned in iOS docs
    // and use underscores in their names - they're most probably
    // are considered private and therefore should be avoided
    // Here is another way to get arguments using Objective C
    // runtime
    //
    // In general it looks like:
    // res = Vec::new()
    // let args = [[NSProcessInfo processInfo] arguments]
    // for i in (0..[args count])
    //      res.push([args objectAtIndex:i])
    // res

    // TODO
    // But does anyone really need it?
    #[cfg(any(target_os = "ios", target_os = "watchos"))]
    pub fn addr() -> Result<MemInfo, Error> {
        Error(InvalidArgvPointerError())
    }
    /*
    pub fn args() -> Args {
        use crate::ffi::OsString;
        use crate::mem;
        use crate::str;

        extern "C" {
            fn sel_registerName(name: *const libc::c_uchar) -> Sel;
            fn objc_getClass(class_name: *const libc::c_uchar) -> NsId;
        }

        #[cfg(target_arch = "aarch64")]
        extern "C" {
            fn objc_msgSend(obj: NsId, sel: Sel) -> NsId;
            #[allow(clashing_extern_declarations)]
            #[link_name = "objc_msgSend"]
            fn objc_msgSend_ul(obj: NsId, sel: Sel, i: libc::c_ulong) -> NsId;
        }

        #[cfg(not(target_arch = "aarch64"))]
        extern "C" {
            fn objc_msgSend(obj: NsId, sel: Sel, ...) -> NsId;
            #[allow(clashing_extern_declarations)]
            #[link_name = "objc_msgSend"]
            fn objc_msgSend_ul(obj: NsId, sel: Sel, ...) -> NsId;
        }

        type Sel = *mut libc::c_void;
        type NsId = *mut libc::c_void;

        let mut res = Vec::new();

        unsafe {
            let process_info_sel = sel_registerName("processInfo\0".as_ptr());
            let arguments_sel = sel_registerName("arguments\0".as_ptr());
            let utf8_sel = sel_registerName("UTF8String\0".as_ptr());
            let count_sel = sel_registerName("count\0".as_ptr());
            let object_at_sel = sel_registerName("objectAtIndex:\0".as_ptr());

            let klass = objc_getClass("NSProcessInfo\0".as_ptr());
            let info = objc_msgSend(klass, process_info_sel);
            let args = objc_msgSend(info, arguments_sel);

            let cnt: usize = mem::transmute(objc_msgSend(args, count_sel));
            for i in 0..cnt {
                let tmp = objc_msgSend_ul(args, object_at_sel, i as libc::c_ulong);
                let utf_c_str: *mut libc::c_char = mem::transmute(objc_msgSend(tmp, utf8_sel));
                let bytes = CStr::from_ptr(utf_c_str).to_bytes();
                res.push(OsString::from(str::from_utf8(bytes).unwrap()))
            }
        }

        Args { iter: res.into_iter() }
    }
    */
}