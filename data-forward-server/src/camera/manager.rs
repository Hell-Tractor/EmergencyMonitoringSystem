use std::collections::HashMap;

use super::camera::{ClosedCamera, OpenedCamera};
use super::camera;
use super::api;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Camera `{0}` is not found or already opened.")]
    ClosedCameraNotFound(String),
    #[error(transparent)]
    CameraError(#[from] camera::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct CameraManager {
    closed_cameras: HashMap<String, ClosedCamera>,
    opened_cameras: HashMap<String, OpenedCamera>,
}

impl CameraManager {
    pub fn new() -> Self {
        let dev_num = unsafe { api::cam_enum_devices() };
        let devices = unsafe { api::cam_get_device_list() };
        let mut closed_cameras = HashMap::new();
        for i in 0..dev_num {
            let info = unsafe { (*devices.offset(i as isize)).clone() };
            closed_cameras.insert(info.device, ClosedCamera { info: info.into() });
        }
        todo!()
    }

    pub fn get_camera_list(&self) -> Vec<String> {
        self.closed_cameras.iter().map(|(key, _)| key.clone())
            .chain(self.opened_cameras.iter().map(|(key, _)| key.clone()))
            .collect()
    }

    pub fn open_camera(&mut self, dev_name: &str, fmt_index: usize, res_index: usize, fps_index: usize) -> Result<()> {
        if let Some(closed_camera) = self.closed_cameras.remove(dev_name) {
            let opened_camera = closed_camera.open(fmt_index, res_index, fps_index)?;
            self.opened_cameras.insert(dev_name.to_string(), opened_camera);
            Ok(())
        } else {
            Err(Error::ClosedCameraNotFound(dev_name.to_string()))
        }
    }

    pub fn grab_frame(&self, dev_name: &str) -> Result<Vec<u8>> {
        if let Some(opened_camera) = self.opened_cameras.get(dev_name) {
            let camera = opened_camera.play()?;
            let frame = camera.grab_frame()?;
            camera.stop()?;
            Ok(frame)
        } else {
            Err(Error::ClosedCameraNotFound(dev_name.to_string()))
        }
    }
}