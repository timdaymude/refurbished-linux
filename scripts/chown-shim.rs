// Build-time shim: inside an unprivileged user namespace only one uid/gid is
// mapped, so any attempt to chown a file to a daemon user or group fails with
// EINVAL and takes the package's maintainer script down with it. The ISO's
// ownership is normalised to root by `mksquashfs -all-root`, so ignoring chown
// during the build is correct rather than a shortcut.
#![allow(non_camel_case_types)]
use std::os::raw::{c_char, c_int, c_uint};

#[no_mangle]
pub extern "C" fn chown(_path: *const c_char, _owner: c_uint, _group: c_uint) -> c_int { 0 }
#[no_mangle]
pub extern "C" fn lchown(_path: *const c_char, _owner: c_uint, _group: c_uint) -> c_int { 0 }
#[no_mangle]
pub extern "C" fn fchown(_fd: c_int, _owner: c_uint, _group: c_uint) -> c_int { 0 }
#[no_mangle]
pub extern "C" fn fchownat(_fd: c_int, _path: *const c_char, _owner: c_uint, _group: c_uint, _flags: c_int) -> c_int { 0 }
