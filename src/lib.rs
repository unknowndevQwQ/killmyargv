mod argv_addr;
mod env_addr;

#[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
use std::env::{remove_var, set_var, vars_os};
use std::{
    cmp,
    ffi::{c_char, CStr, CString, OsStr},
    os::unix::ffi::OsStrExt,
    slice,
    sync::{Mutex, OnceLock},
};

use log::{debug, error, trace, warn};
use thiserror::Error;

const OS_MAX_LEN_LIMIT: usize = if cfg!(any(target_os = "illumos", target_os = "solaris")) {
    4095
} else if cfg!(all(target_os = "linux", feature = "clobber_environ")) {
    4096
} else {
    usize::MAX
};

static ARGV_MEM: OnceLock<Mutex<MemInfo>> = OnceLock::new();

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("The *argv[] points to an invalid memory address.")]
    InvalidArgvPointer,
    #[error("Failed to get *argv[] pointer.")]
    FailedToGetArgvPointer,
    #[error("Failed to get a string from pointer `{ptr:?}`.")]
    FailedToGetString { ptr: *const *const c_char },
    #[error("The pointer `{ptr:?}` points to null.")]
    PonitToNull { ptr: *const *const c_char },
    #[error("The pointer as null.")]
    NullPointer,
}

unsafe impl Send for EnvError {}

#[derive(Debug)]
pub struct KillMyArgv {
    begin_addr: *mut u8,
    end_addr: *mut u8,
    max_len: usize,
    saved_argv: Vec<CString>,
    nonul_byte: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct MemInfo {
    begin_addr: *const c_char,
    end_addr: *const c_char,
    #[allow(unused)]
    // argv[count] or environ[count]
    count: usize,
    #[allow(unused)]
    ptr: *const *const c_char,
}

unsafe impl Send for MemInfo {}

unsafe fn from_addr(count: usize, ptr: *const *const c_char) -> Result<MemInfo, EnvError> {
    if ptr.is_null() {
        return Err(EnvError::NullPointer);
    } else if ptr.read().is_null() {
        return Err(EnvError::PonitToNull { ptr });
    }

    let mut available = 0;

    //copied from: https://github.com/rust-lang/rust/blob/1.86.0/library/std/src/sys/pal/unix/args.rs#L23-L50
    for i in 0..count {
        // SAFETY: `argv` is non-null if `argc` is positive, and it is
        // guaranteed to be at least as long as `argc`, so reading from it
        // should be safe.
        let in_ptr = unsafe { ptr.add(i).read() };

        // Some C commandline parsers (e.g. GLib and Qt) are replacing already
        // handled arguments in `argv` with `NULL` and move them to the end.
        //
        // Since they can't directly ensure updates to `argc` as well, this
        // means that `argc` might be bigger than the actual number of
        // non-`NULL` pointers in `argv` at this point.
        //
        // To handle this we simply stop iterating at the first `NULL`
        // argument. `argv` is also guaranteed to be `NULL`-terminated so any
        // non-`NULL` arguments after the first `NULL` can safely be ignored.
        if in_ptr.is_null() {
            // NOTE: On Apple platforms, `-[NSProcessInfo arguments]` does not
            // stop iterating here, but instead `continue`, always iterating
            // up until it reached `argc`.
            //
            // This difference will only matter in very specific circumstances
            // where `argc`/`argv` have been modified, but in unexpected ways,
            // so it likely doesn't really matter which option we choose.
            // See the following PR for further discussion:
            // <https://github.com/rust-lang/rust/pull/125225>
            break;
        }

        // SAFETY: Just checked that the pointer is not NULL, and arguments
        // are otherwise guaranteed to be valid C strings.
        trace!("given count={count}, available count={available}, ptr={ptr:?}, current={in_ptr:?}");
        available = i;
    }

    let begin_addr = unsafe { ptr.read() };
    let mut end_addr = unsafe { ptr.add(available).read() };
    end_addr = unsafe { end_addr.add(CStr::from_ptr(end_addr).to_bytes().len()) };

    debug!("given count={count}, available count={available}, ptr={ptr:?}, range: {begin_addr:?} -> {end_addr:?}");
    if count > 0 {
        Ok(MemInfo {
            begin_addr,
            end_addr,
            count: available + 1,
            ptr,
        })
    } else {
        Err(EnvError::FailedToGetString { ptr })
    }
}

// The expected input is always the checked output of from_addr()
fn save_string(count: usize, ptr: *const *const c_char) -> Vec<CString> {
    let mut saved: Vec<CString> = Vec::with_capacity(count);
    let cstr_ptrs = unsafe { slice::from_raw_parts(ptr, count) };
    for (i, cstr_ptr) in cstr_ptrs.into_iter().enumerate() {
        trace!("string[{i}]={cstr_ptr:?}, ptr={:?}", cstr_ptr as *const _);
        if cstr_ptr.is_null() {
            warn!("the string[{i}] is null, pls check");
            break;
        }
        let cstr_len = unsafe { CStr::from_ptr(*cstr_ptr) }
            .to_bytes_with_nul()
            .len();
        saved.push(unsafe { CStr::from_ptr(*cstr_ptr) }.into());
        // Decide elsewhere whether to exclude nul.
        trace!(
            "collect: string[{i}] range(with null): {cstr_ptr:?} -> {:?}, len={cstr_len}",
            unsafe { cstr_ptr.add(cstr_len - 1) }
        );
    }
    saved
}

// Get environ address, ignore errors
fn from_env() -> Option<MemInfo> {
    let (count, ptr) = env_addr::addr()?;
    unsafe { from_addr(count, ptr) }
        .map_err(|e| {
            trace!("env err: {e:?}");
            e
        })
        .ok()
}

fn from_argv() -> Result<MemInfo, EnvError> {
    let argv_info = match ARGV_MEM.get() {
        None => {
            let (count, ptr) = argv_addr::addr()?;
            let argv_info = unsafe { from_addr(count, ptr) }.map_err(|e| {
                trace!("argv err: {e:?}");
                e
            })?;
            ARGV_MEM.get_or_init(|| Mutex::new(argv_info))
        }
        Some(val) => val,
    };
    Ok(*argv_info.lock().unwrap())
}

/// Get the argv start address and end address.
pub fn argv_addrs() -> Result<(*mut u8, *mut u8), EnvError> {
    from_argv().map(|m| (m.begin_addr as *mut u8, m.end_addr as *mut u8))
}

/// Get raw args count and args pointer
/// # Safety
/// The string address is changed after KillMyArgv::new() with the replace_argv_element feature enabled.
pub unsafe fn argc_argv() -> Result<(usize, *const *const c_char), EnvError> {
    argv_addr::addr()
}

/// Get raw environ pointer
/// # Safety
/// See: <https://doc.rust-lang.org/stable/std/env/fn.set_var.html#safety>
pub unsafe fn environ() -> Option<*const *const c_char> {
    env_addr::envptr()
}

impl KillMyArgv {
    /// Get the argv start address and end address.
    pub fn argv_addrs(&self) -> (*mut u8, *mut u8) {
        (self.begin_addr, self.end_addr)
    }

    pub fn new() -> Result<KillMyArgv, EnvError> {
        debug!("current target: {}", env!("TARGET"));
        let argv_mem = from_argv()?;
        let argv_saved = save_string(argv_mem.count, argv_mem.ptr);
        // It can be replaced by std::ptr::sub_ptr() in the future.
        let argv_len = unsafe { argv_mem.end_addr.offset_from(argv_mem.begin_addr) as usize };

        trace!("argv struct: {argv_mem:#?}, saved: {argv_saved:#?}, len={argv_len}");
        if cfg!(feature = "replace_argv_element") {
            let mut new_argvp = argv_saved
                .clone()
                .leak()
                .iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<*const c_char>>();

            for i in (0..argv_mem.count).rev() {
                if let Some(new_ptr) = new_argvp.pop() {
                    debug!("processing argv[{i}], try set new ptr={new_ptr:?}");
                    unsafe {
                        let ptr = argv_mem.ptr.add(i) as *mut *const c_char;
                        trace!(
                            "argv[{i}]={ptr:?}, point to: {:?}, set point to: {new_ptr:?}",
                            *ptr
                        );
                        ptr.write(new_ptr);
                    }
                } else {
                    warn!("new_argvp ptr is none");
                }
            }
        }
        if argv_len - 1 < OS_MAX_LEN_LIMIT {
            if let Some(env_mem) = from_env() {
                let env_saved = save_string(env_mem.count, env_mem.ptr);
                let env_len = unsafe { env_mem.end_addr.offset_from(env_mem.begin_addr) as usize };
                trace!("env struct: {env_mem:#?}, saved: {env_saved:#?}, len={env_len}");
                #[allow(unused)]
                #[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
                // I haven't decided if I want to remove it or not,
                // since setenv makes it probably unnecessary.
                let mut new_envp = env_saved
                    .iter()
                    .map(|s| s.as_ptr())
                    .collect::<Vec<*const c_char>>();

                // Using std instead of manually replacing each element in environ
                // is just being lazy.
                #[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
                for (key, value) in vars_os() {
                    remove_var(&key);
                    set_var(key, value); // Expected: libc::setenv(key, value, 1)
                }

                return Ok(KillMyArgv {
                    begin_addr: argv_mem.begin_addr as *mut u8,
                    end_addr: env_mem.end_addr as *mut u8,
                    max_len: cmp::min(argv_len + 1 + env_len, OS_MAX_LEN_LIMIT),
                    saved_argv: argv_saved,
                    nonul_byte: Some(argv_len),
                });
            }
        }
        Ok(KillMyArgv {
            begin_addr: argv_mem.begin_addr as *mut u8,
            end_addr: argv_mem.end_addr as *mut u8,
            max_len: if cfg!(any(target_os = "illumos", target_os = "solaris")) {
                cmp::min(argv_len, OS_MAX_LEN_LIMIT)
            } else {
                argv_len
            },
            saved_argv: argv_saved,
            nonul_byte: None,
        })
    }

    /// Gets the maximum byte length for which the cmdline can be set.
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// origin argv length, containing the terminating NUL byte.
    /// This bit is written to a non-nul value requiring attention to os behavior.
    pub fn nonul_byte(&self) -> Option<usize> {
        self.nonul_byte
    }

    /// Undo the args/cmdline changes.
    pub fn revert(&self) {
        let backup_chars: Vec<u8> = self
            .saved_argv
            .iter()
            .flat_map(|s| s.as_bytes_with_nul())
            .cloned()
            .collect();
        Self::set(self, &backup_chars)
    }

    /// set a new args/cmdline.
    pub fn set(&self, chars: &[u8]) {
        trace!(
            "set len: {:?}, need not null byte: {:?}, String: {:?}, bytes hex: {chars:02x?}",
            chars.len(),
            self.nonul_byte,
            OsStr::from_bytes(chars)
        );
        unsafe {
            if chars.len() < self.max_len {
                slice::from_raw_parts_mut(self.begin_addr, chars.len()).copy_from_slice(chars);

                self.begin_addr
                    .add(chars.len())
                    .write_bytes(0x00, self.max_len - chars.len());
            } else {
                slice::from_raw_parts_mut(self.begin_addr, self.max_len)
                    .copy_from_slice(&chars[..self.max_len]);
            }
            // It should be handled by advanced packaging or users,
            // and is difficultto dispose of properly here.
            if let Some(nonul_byte) = self.nonul_byte {
                if chars.len() > nonul_byte && self.begin_addr.add(nonul_byte - 1).read() == 0x00 {
                    warn!(
                        "Note! you try in nonul byte({nonul_byte}) write null, {}, {}",
                        "because there is currently no corresponding API to decide whether to fully follow the written content",
                        "it is replaced by 0x01."
                    );
                    self.begin_addr.add(nonul_byte - 1).write(0x01);
                }
            }
            let end = self.end_addr.read();
            if end != 0x00 {
                error!("BUG! Unexpected non-null value: {end:?}");
            }
        }
    }
}
