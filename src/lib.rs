mod argv_addr;
mod env_addr;

#[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
use std::env::{remove_var, set_var, vars_os};
use std::{
    cmp,
    ffi::{c_char, CStr, CString, OsStr},
    os::unix::ffi::OsStrExt,
    slice,
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

#[derive(Debug)]
pub struct KillMyArgv {
    begin_addr: *mut u8,
    end_addr: *mut u8,
    max_len: usize,
    saved_argv: Vec<CString>,
    nonul_byte: Option<usize>,
}

#[derive(Debug)]
struct MemInfo {
    begin_addr: *const c_char,
    end_addr: *const c_char,
    byte_len: usize,
    #[allow(unused)]
    // argv[element] or environ[element]
    element: usize,
    saved: Vec<CString>,
    #[allow(unused)]
    pointer_addr: *const *const c_char,
}

unsafe fn from_addr(count: usize, ptr: *const *const c_char) -> Result<MemInfo, EnvError> {
    if ptr.is_null() {
        return Err(EnvError::NullPointer);
    } else if (*ptr).is_null() {
        return Err(EnvError::PonitToNull { ptr });
    }

    let mut byte_len = 0;
    let mut end_addr = *ptr;
    let mut saved: Vec<CString> = Vec::with_capacity(count);
    let cstr_ptrs = slice::from_raw_parts(ptr, count);
    let cstr_ptrs_len = cstr_ptrs.len();
    if cstr_ptrs_len != count {
        warn!(
            "The actual length of the array ({cstr_ptrs_len}) does not match the argument ({count})."
        );
    }
    for (i, cstr_ptr) in cstr_ptrs.into_iter().enumerate() {
        trace!("string[{i}]={cstr_ptr:?}, ptr={:?}", cstr_ptr as *const _);
        if cstr_ptr.is_null() {
            warn!("the string[{i}] is null, pls check");
            break;
        }
        let cstr_len = CStr::from_ptr(*cstr_ptr).to_bytes_with_nul().len();
        saved.push(CStr::from_ptr(*cstr_ptr).into());
        // Decide elsewhere whether to exclude nul.
        byte_len += cstr_len;

        trace!(
            "collect: string[{i}] recorded len={byte_len}, range(with null): {cstr_ptr:?} -> {:?}, len={cstr_len}",
            cstr_ptr.add(cstr_len - 1)
        );
        if i == count - 1 {
            // Perhaps it would be better to add 1 byte manually when
            // calculating the length.
            // Avoid overstepping the bounds.
            end_addr = cstr_ptr.add(cstr_len - 1);
        }
    }
    debug!(
        "collect count={count}, ptr={ptr:?}, range: {:?} -> {end_addr:?}, len={byte_len}, vec_cap={}",
        *ptr, saved.capacity()
    );
    if byte_len != 0 {
        Ok(MemInfo {
            begin_addr: *ptr,
            end_addr,
            byte_len,
            element: count,
            saved,
            pointer_addr: ptr,
        })
    } else {
        Err(EnvError::FailedToGetString { ptr })
    }
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
    let (count, ptr) = argv_addr::addr()?;
    unsafe { from_addr(count, ptr) }.map_err(|e| {
        trace!("argv err: {e:?}");
        e
    })
}

/// Get the argv start address and end address.
pub fn argv_addrs() -> Result<(*mut u8, *mut u8), EnvError> {
    from_argv().map(|m| (m.begin_addr as *mut u8, m.end_addr as *mut u8))
}

impl KillMyArgv {
    /// Get the argv start address and end address.
    pub fn argv_addrs(&self) -> (*mut u8, *mut u8) {
        (self.begin_addr, self.end_addr)
    }

    pub fn new() -> Result<KillMyArgv, EnvError> {
        debug!("current target: {}", env!("TARGET"));
        let argv_mem = from_argv()?;

        trace!("argv struct: {argv_mem:#?}");
        if cfg!(feature = "replace_argv_element") {
            let mut new_argvp = argv_mem
                .saved
                .clone()
                .leak()
                .iter()
                .map(|s| s.as_ptr())
                .collect::<Vec<*const c_char>>();

            for i in (0..argv_mem.element).rev() {
                if let Some(new_ptr) = new_argvp.pop() {
                    debug!("processing argv[{i}], try set new ptr={new_ptr:?}");
                    unsafe {
                        let ptr = argv_mem.pointer_addr.add(i) as *mut *const c_char;
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
        if argv_mem.byte_len - 1 < OS_MAX_LEN_LIMIT {
            if let Some(env_mem) = from_env() {
                trace!("env struct: {env_mem:#?}");
                #[allow(unused)]
                #[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
                // I haven't decided if I want to remove it or not,
                // since setenv makes it probably unnecessary.
                let mut new_envp = env_mem
                    .saved
                    .leak()
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
                    max_len: cmp::min(argv_mem.byte_len + env_mem.byte_len - 1, OS_MAX_LEN_LIMIT),
                    saved_argv: argv_mem.saved,
                    nonul_byte: Some(argv_mem.byte_len),
                });
            }
        }
        Ok(KillMyArgv {
            begin_addr: argv_mem.begin_addr as *mut u8,
            end_addr: argv_mem.end_addr as *mut u8,
            max_len: if cfg!(any(target_os = "illumos", target_os = "solaris")) {
                cmp::min(argv_mem.byte_len - 1, OS_MAX_LEN_LIMIT)
            } else {
                argv_mem.byte_len - 1
            },
            saved_argv: argv_mem.saved,
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
