use libc::c_char;

pub const MAIN_DEV: i32 = 0;

#[repr(C)]
#[derive(Clone)]
pub struct CFpsInfo {
    pub den: u32,
    pub num: u32,
} // C: CameraAPI.h:25, __fpsInfo

#[repr(C)]
#[derive(Clone)]
pub struct CResInfo {
    pub w: u32,
    pub h: u32,
    pub fps_list: *mut CFpsInfo,
    pub fps_num: i32,
} // C: CameraAPI.h:30, __resInfo

#[repr(C)]
#[derive(Clone)]
pub struct CFmtInfo {
    pub desc: [c_char; 32],
    pub pixfmt: u32,
    pub res_list: *mut CResInfo,
    pub res_num: i32,
} // C: CameraAPI.h:37, __fmtInfo

#[repr(C)]
#[derive(Clone)]
pub struct CDevInfo {
    pub device: [c_char; 256],
    pub name: [c_char; 256],
    pub manufacture: [c_char; 256],
    pub product: [c_char; 256],
    pub serial: [c_char; 256],
    pub vid: i32,
    pub pid: i32,
    pub fmt_list: *mut CFmtInfo,
    pub fmt_num: i32,
} // C: CameraAPI.h:44, __devInfo

#[repr(C)]
#[derive(Clone)]
pub struct CVideoCtrl {
    pub min: i32,
    pub max: i32,
    pub deft: i32,
    pub step: i32,
    pub flag: u32,
} // C: CameraAPI.h:51, __videoCtrl

#[allow(dead_code)]
unsafe extern "C" {
    pub(super) fn cam_enum_devices() -> i32;
    pub(super) fn cam_free_devices();
    pub(super) fn cam_get_device_num() -> i32;
    pub(super) fn cam_get_device_list() -> *mut CDevInfo;
    pub(super) fn cam_open(dev: i32, dev_name: *const c_char, pix_fmt: u32, w: u32, h: u32, fps_den: u32, fps_num: u32) -> i32;
    pub(super) fn cam_close(dev: i32);
    pub(super) fn cam_play(dev: i32) -> i32;
    pub(super) fn cam_stop(dev: i32) -> i32;
    pub(super) fn cam_grab_frame(dev: i32, frame: libc::c_uchar, len: i32) -> i32;
    pub(super) fn cam_control_query(dev: i32, ctrl_id: u32, pctrl: *mut CVideoCtrl) -> i32;
    pub(super) fn cam_control_set(dev: i32, ctrl_id: u32, value: i32) -> i32;
    pub(super) fn cam_control_get(dev: i32, ctrl_id: u32, value: *mut i32) -> i32;
    pub(super) fn cam_dsp_read_buf(dev: i32, adr: libc::c_ushort, pval: *mut libc::c_uchar, vallen: i32) -> i32;
    pub(super) fn cam_dsp_read(dev: i32, adr: libc::c_ushort, val: *mut libc::c_uchar) -> i32;
    pub(super) fn cam_dsp_write_buf(dev: i32, adr: libc::c_ushort, pval: *mut libc::c_uchar, vallen: i32) -> i32;
    pub(super) fn cam_dsp_write(dev: i32, adr: libc::c_ushort, val: libc::c_uchar) -> i32;
    pub(super) fn cam_sensor_read16(dev: i32, adr: libc::c_ushort, val: *mut libc::c_ushort) -> i32;
    pub(super) fn cam_sensor_read8(dev: i32, adr: libc::c_ushort, val: *mut libc::c_uchar) -> i32;
    pub(super) fn cam_sensor_write16(dev: i32, adr: libc::c_ushort, val: libc::c_ushort) -> i32;
    pub(super) fn cam_sensor_write8(dev: i32, adr: libc::c_ushort, val: libc::c_uchar) -> i32;
}