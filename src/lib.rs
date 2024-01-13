mod argv_addr;
mod env_addr;

#[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
use std::env::{set_var, vars_os};
use std::{
    ffi::{c_char, CStr, CString, OsStr},
    os::unix::ffi::OsStrExt,
    ptr, slice,
};

use log::{error, trace};
use thiserror::Error;

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
    for i in 0..count {
        let cstr_ptr = *ptr.add(i);
        let cstr_len = CStr::from_ptr(cstr_ptr).to_bytes_with_nul().len();
        saved.push(CStr::from_ptr(cstr_ptr).into());
        if i < count {
            // Decide elsewhere whether to exclude nul.
            byte_len += cstr_len;

            trace!("string[{i}] collect: recorded len={byte_len}, ptr={cstr_ptr:?}, string len={cstr_len}, next ptr={:?}",
                cstr_ptr.add(cstr_len - 1)
            );
            if i == count - 1 {
                // It is assumed that arg/environ must never have an element
                // of length 0, otherwise unpredictable results would occur.
                // Perhaps it would be better to add 1 byte manually when
                // calculating the length.
                // Avoid overstepping the bounds.
                end_addr = cstr_ptr.add(cstr_len - 1);
            }
        }
    }
    trace!(
        "collect count: {count}, string ptr: {ptr:?}, addr: {:?} -> {end_addr:?}, len: {byte_len}, vec_cap: {}",
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
    match env_addr::addr() {
        Some((count, ptr)) => unsafe {
            match from_addr(count, ptr) {
                Ok(m) => Some(m),
                Err(e) => {
                    trace!("{:?}", e);
                    None
                }
            }
        },
        None => None,
    }
}

fn from_argv() -> Result<MemInfo, EnvError> {
    match argv_addr::addr() {
        Ok((count, ptr)) => unsafe {
            match from_addr(count, ptr) {
                Ok(m) => Ok(m),
                Err(e) => {
                    trace!("{:?}", e);
                    Err(e)
                }
            }
        },
        Err(e) => Err(e),
    }
}

impl KillMyArgv {
    /// Get the argv start address and end address.
    pub unsafe fn argv_addrs() -> Result<(*mut u8, *mut u8), EnvError> {
        match from_argv() {
            Ok(v) => Ok((v.begin_addr as *mut u8, v.end_addr as *mut u8)),
            Err(e) => Err(e),
        }
    }

    pub fn new() -> Result<KillMyArgv, EnvError> {
        match (from_argv(), from_env()) {
            (Ok(argv_mem), None) => {
                trace!("argv struct: {argv_mem:#?}");
                #[cfg(feature = "replace_argv_element")]
                let mut new_argvp = argv_mem
                    .saved
                    .iter()
                    .map(|s| s.as_ptr())
                    .collect::<Vec<*const c_char>>();

                #[cfg(feature = "replace_argv_element")]
                for i in (0..argv_mem.element).rev() {
                    if let Some(new_ptr) = new_argvp.pop() {
                        trace!("processing argc: {i}, ptr: {new_ptr:?}");
                        unsafe {
                            let ptr = argv_mem.pointer_addr as *mut *const c_char;
                            ptr.add(i).write(new_ptr);
                            trace!("ptrs: {:?}, {:?}, {new_ptr:?}", *ptr.add(i), ptr.add(i));
                        }
                    } else {
                        trace!("argv ptr as empty");
                    };
                }

                Ok(KillMyArgv {
                    begin_addr: argv_mem.begin_addr as *mut u8,
                    end_addr: argv_mem.end_addr as *mut u8,
                    max_len: argv_mem.byte_len - 1,
                    saved_argv: argv_mem.saved,
                    nonul_byte: None,
                })
            }
            (Ok(argv_mem), Some(env_mem)) => {
                trace!("argv struct: {argv_mem:#?}, env struct: {env_mem:#?}");
                #[cfg(feature = "replace_argv_element")]
                let mut new_argvp = argv_mem
                    .saved
                    .iter()
                    .map(|s| s.as_ptr())
                    .collect::<Vec<*const c_char>>();

                #[cfg(feature = "replace_argv_element")]
                for i in (0..argv_mem.element).rev() {
                    if let Some(new_ptr) = new_argvp.pop() {
                        trace!("processing argc: {i}, ptr: {new_ptr:?}");
                        unsafe {
                            let ptr = argv_mem.pointer_addr as *mut *const c_char;
                            trace!("ptrs: {:?}, {:?}, {new_ptr:?}", *ptr.add(i), ptr.add(i));
                            ptr.add(i).write(new_ptr);
                        }
                    } else {
                        trace!("new_argvp as none");
                    };
                }

                #[allow(unused)]
                #[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
                // I haven't decided if I want to remove it or not,
                // since setenv makes it probably unnecessary.
                let mut new_envp = env_mem
                    .saved
                    .iter()
                    .map(|s| s.as_ptr())
                    .collect::<Vec<*const c_char>>();

                // Using std instead of manually replacing each element in environ
                // is just being lazy.
                #[cfg(all(feature = "clobber_environ", feature = "replace_environ_element"))]
                for (key, value) in vars_os() {
                    set_var(&key, "NoNe"); // This line may not be needed.
                    set_var(key, value); // Expected: libc::setenv(key, value, 1)
                }

                Ok(KillMyArgv {
                    begin_addr: argv_mem.begin_addr as *mut u8,
                    end_addr: env_mem.end_addr as *mut u8,
                    max_len: argv_mem.byte_len + env_mem.byte_len - 1,
                    saved_argv: argv_mem.saved,
                    nonul_byte: Some(argv_mem.byte_len),
                })
            }
            (Err(e), _) => Err(e),
        }
    }

    /// Undo the args/cmdline changes.
    pub fn revert(&self) {
        let backup_chars: Vec<u8> = self
            .saved_argv
            .iter()
            .map(|s| s.as_bytes_with_nul())
            .flatten()
            .cloned()
            .collect();
        Self::set(&self, &backup_chars)
    }

    /// set a new args/cmdline.
    pub fn set(&self, chars: &[u8]) {
        trace!(
            "set len: {:?}, need not null byte: {:?}, String: {:?}, bytes hex: {chars:02x?}",
            chars.len(),
            self.nonul_byte,
            OsStr::from_bytes(&chars)
        );
        unsafe {
            if chars.len() < self.max_len {
                slice::from_raw_parts_mut(self.begin_addr, chars.len()).copy_from_slice(&chars[..]);

                ptr::write_bytes(
                    self.begin_addr.add(chars.len()),
                    0x00,
                    self.max_len - chars.len(),
                );
            } else {
                slice::from_raw_parts_mut(self.begin_addr, self.max_len)
                    .copy_from_slice(&chars[..self.max_len]);
            }
            // It should be handled by advanced packaging or users,
            // and is difficultto dispose of properly here.
            if let Some(nonul_byte) = self.nonul_byte {
                if chars.len() > nonul_byte
                    && dbg!(ptr::read(self.begin_addr.add(nonul_byte - 1))) == 0x00
                {
                    dbg!(ptr::write_bytes(
                        self.begin_addr.add(nonul_byte - 1),
                        0x01,
                        1
                    ));
                }
            }
            let end = ptr::read(self.end_addr);
            if end != 0x00 {
                error!("BUG! Unexpected non-null value: {end:?}");
            }
        }
    }
}
