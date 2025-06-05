use crate::camera::{adapter::{self, DevInfo}, api};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Index out of bounds of array `{0}`. length: {1}, given index: {2}.")]
    IndexOutOfBounds(String, usize, usize),
    #[error("Failed to open camera `{0}`. Error code: {1}.")]
    OpenCameraFailed(String, i32),
}

type Result<T> = std::result::Result<T, Error>;

pub struct ClosedCamera {
    pub info: DevInfo,
}

pub struct OpenedCamera {
    pub info: DevInfo,
}

pub struct PlayingCamera {
    pub info: DevInfo,
}

impl ClosedCamera {
    pub fn open(self, fmt_index: usize, res_index: usize, fps_index: usize) -> Result<OpenedCamera> {
        let dev_name = &self.info.device;
        let fmt_info = self.info.fmt_list.get(fmt_index).ok_or(Error::IndexOutOfBounds("fmt_list".to_string(), self.info.fmt_list.len(), fmt_index))?;
        let pix_fmt = fmt_info.pixfmt;
        let res_info = fmt_info.res_list.get(res_index).ok_or(Error::IndexOutOfBounds("res_list".to_string(), fmt_info.res_list.len(), res_index))?;
        let w = res_info.w;
        let h = res_info.h;
        let fps_info = res_info.fps_list.get(fps_index).ok_or(Error::IndexOutOfBounds("fps_list".to_string(), res_info.fps_list.len(), fps_index))?;
        let den = fps_info.den;
        let num = fps_info.num;

        unsafe {
            let dev_name_ptr = adapter::string_to_c_str(&dev_name);
            let result = api::cam_open(api::MAIN_DEV, dev_name_ptr, pix_fmt, w, h, den, num);
            if result < 0 {
                return Err(Error::OpenCameraFailed(dev_name.clone(), result));
            }
            adapter::free_c_str(dev_name_ptr);
        }

        return Ok(OpenedCamera {
            info: self.info,
        });
    }
}

impl OpenedCamera {
    pub fn close(self) -> ClosedCamera {
        todo!()
    }
    pub fn play(self) -> PlayingCamera {
        todo!()
    }
}

impl PlayingCamera {
    pub fn stop(self) -> OpenedCamera {
        todo!()
    }
    pub fn close(self) -> ClosedCamera {
        self.stop().close()
    }
}