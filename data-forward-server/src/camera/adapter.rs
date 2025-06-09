use std::ffi::{CStr, CString};

use libc::c_char;

use super::api::{CDevInfo, CFmtInfo, CFpsInfo, CResInfo, CVideoCtrl};

pub type FpsInfo = CFpsInfo;

pub struct ResInfo {
    pub w: u32,
    pub h: u32,
    pub fps_list: Vec<FpsInfo>,
}

pub struct FmtInfo {
    pub desc: String,
    pub pixfmt: u32,
    pub res_list: Vec<ResInfo>,
}

pub struct DevInfo {
    pub device: String,
    pub name: String,
    pub manufacture: String,
    pub product: String,
    pub serial: String,
    pub vid: i32,
    pub pid: i32,
    pub fmt_list: Vec<FmtInfo>,
}

pub struct VideoCtrl {
    pub min: i32,
    pub max: i32,
    pub deft: i32,
    pub step: i32,
    pub flag: u32,
}

/// Converts a C pointer to a vector of type T by *CLONE*.
///
/// # Safety
/// The caller must ensure that the pointer is valid and points to a contiguous block of memory containing `len` elements of type `T`.
pub unsafe fn c_ptr_to_vec<T: Clone>(ptr: *mut T, len: i32) -> Vec<T> {
    if ptr.is_null() || len <= 0 {
        return Vec::new();
    }
    let mut vec = Vec::with_capacity(len as usize);
    unsafe {
        for i in 0..len {
            vec.push((*ptr.offset(i as isize)).clone());
        }
    }
    vec
}

/// Converts a `Vec<T>` to a C pointer, returning the pointer, length, and capacity.
///
/// # Safety
/// The caller must ensure that the `Vec<T>` is not empty, as an empty vector will return a null pointer.
/// The caller is responsible for freeing the memory allocated for the pointer using `free_c_ptr`.
pub unsafe fn vec_to_c_ptr<T>(vec: Vec<T>) -> (*mut T, i32, i32) {
    if vec.is_empty() {
        return (std::ptr::null_mut(), 0, 0);
    }
    let cap = vec.capacity() as i32;
    let len = vec.len() as i32;
    let mut c_vec = vec.into_boxed_slice();
    let ptr = c_vec.as_mut_ptr();
    std::mem::forget(c_vec); // Prevent deallocation
    (ptr, len, cap)
}

/// Frees a C pointer that was allocated by `vec_to_c_ptr`.
///
/// # Safety
/// The caller must ensure that the pointer was allocated using `vec_to_c_ptr` and that it is not used after this function is called.
/// The capacity (`cap`) should match the original capacity used when the pointer was created.
pub unsafe fn free_c_ptr<T>(ptr: *mut T, cap: i32) {
    if !ptr.is_null() {
        let _ = unsafe { Vec::from_raw_parts(ptr, 0, cap as usize) };
        // The memory will be freed when the Vec is dropped
    }
}

/// Converts a C-style string (null-terminated) to a Rust `String`.
///
/// # Safety
/// The caller must ensure that the C-style string is valid and null-terminated.
/// The function will return `None` if the string is not valid UTF-8.
pub unsafe fn c_str_to_string(c_str: &[c_char]) -> Option<String> {
    let c_ptr = c_str.as_ptr();
    let c_str = unsafe { CStr::from_ptr(c_ptr) };
    match c_str.to_str() {
        Ok(s) => Some(s.to_string()),
        Err(_) => None,
    }
}

/// Converts a Rust `String` to a C-style string (null-terminated).
///
/// # Safety
/// The caller must ensure that the string does not contain null bytes (`\0`) other than the one added at the end.
/// The returned pointer must be freed using `free_c_str` to avoid memory leaks.
pub unsafe fn string_to_c_str(s: &str) -> *mut c_char {
    let c_str = CString::new(s).expect("Failed to create CString from string");
    c_str.into_raw()
}

/// Frees a C-style string that was allocated by `string_to_c_str`.
///
/// # Safety
/// The caller must ensure that the pointer was allocated using `string_to_c_str` and that it is not used after this function is called.
/// The memory will be deallocated when this function is called.
pub unsafe fn free_c_str(c_str: *mut c_char) {
    if !c_str.is_null() {
        unsafe { CString::from_raw(c_str) }; // This will deallocate the memory
    }
}

impl From<CResInfo> for ResInfo {
    fn from(c_res: CResInfo) -> Self {
        let fps_list = unsafe {
            c_ptr_to_vec(c_res.fps_list, c_res.fps_num)
        };
        ResInfo {
            w: c_res.w,
            h: c_res.h,
            fps_list,
        }
    }
}

impl From<CFmtInfo> for FmtInfo {
    fn from(c_fmt: CFmtInfo) -> Self {
        let res_list = unsafe {
            c_ptr_to_vec(c_fmt.res_list, c_fmt.res_num)
        };
        let res_list = res_list.into_iter().map(ResInfo::from).collect();
        FmtInfo {
            desc: unsafe { c_str_to_string(c_fmt.desc.as_ref()).expect("Failed to decode fmtInfo.desc from C str") },
            pixfmt: c_fmt.pixfmt,
            res_list,
        }
    }
}

impl From<CDevInfo> for DevInfo {
    fn from(c_dev: CDevInfo) -> Self {
        let fmt_list = unsafe {
            c_ptr_to_vec(c_dev.fmt_list, c_dev.fmt_num)
        };
        let fmt_list = fmt_list.into_iter().map(FmtInfo::from).collect();
        DevInfo {
            device: unsafe { c_str_to_string(c_dev.device.as_ref()).expect("Failed to decode devInfo.device from C str") },
            name: unsafe { c_str_to_string(c_dev.name.as_ref()).expect("Failed to decode devInfo.name from C str") },
            manufacture: unsafe { c_str_to_string(c_dev.manufacture.as_ref()).expect("Failed to decode devInfo.manufacture from C str") },
            product: unsafe { c_str_to_string(c_dev.product.as_ref()).expect("Failed to decode devInfo.product from C str") },
            serial: unsafe { c_str_to_string(c_dev.serial.as_ref()).expect("Failed to decode devInfo.serial from C str") },
            vid: c_dev.vid,
            pid: c_dev.pid,
            fmt_list,
        }
    }
}

impl From<CVideoCtrl> for VideoCtrl {
    fn from(c_ctrl: CVideoCtrl) -> Self {
        VideoCtrl {
            min: c_ctrl.min,
            max: c_ctrl.max,
            deft: c_ctrl.deft,
            step: c_ctrl.step,
            flag: c_ctrl.flag,
        }
    }
}