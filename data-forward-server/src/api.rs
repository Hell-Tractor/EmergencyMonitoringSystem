use std::{collections::HashMap, sync::{Arc, Mutex}, time::Duration};

use actix::{Actor, Addr, AsyncContext};
use actix_web::{get, web, HttpResponse, ResponseError, Result};
use tracing::{error, info, warn};
use uuid::Uuid;
use opencv::{
    core,
    imgcodecs,
    imgproc,
    prelude::*,
    videoio,
};
use std::process::Command;
use std::fs;
use std::path::Path;

use crate::web_socket_actor::{DataProcessorWs, ProcessImageForWs};

pub struct AppState {
    pub data_processors: Arc<Mutex<Vec<Addr<DataProcessorWs>>>>,
    pub pending_requests: Arc<Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<Vec<u8>>>>>,
    pub next_processor_idx: Arc<Mutex<usize>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No available data processors found")]
    NoAvailableProcessors,
    #[error("Failed to transfer image data")]
    ImageTransferError,
    #[error("Request timeout")]
    Timeout,
    #[error("OpenCV capture failed")]
    OpenCvCaptureFailed,
    #[error("OpenCV encode failed")]
    OpenCvEncodeFailed,
}

// pub type Result<T> = std::result::Result<T, Error>;

/// 采集摄像头图片并编码为 JPEG
fn capture_jpeg(id: i32) -> std::result::Result<Vec<u8>, Error> {
    let mut cam = videoio::VideoCapture::new(id, videoio::CAP_ANY)
        .map_err(|_| Error::OpenCvCaptureFailed)?;
    if !cam.is_opened().map_err(|_| Error::OpenCvCaptureFailed)? {
        return Err(Error::OpenCvCaptureFailed);
    }
    let mut frame = Mat::default();
    cam.read(&mut frame).map_err(|_| Error::OpenCvCaptureFailed)?;
    if frame.empty() {
        return Err(Error::OpenCvCaptureFailed);
    }
    // 可选：resize 到 640x640
    let mut resized = Mat::default();
    imgproc::resize(
        &frame,
        &mut resized,
        core::Size::new(640, 640),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    ).map_err(|_| Error::OpenCvCaptureFailed)?;
    // 编码为 JPEG
    let mut buf = opencv::core::Vector::<u8>::new();
    imgcodecs::imencode(".jpg", &resized, &mut buf, &opencv::core::Vector::<i32>::new())
        .map_err(|_| Error::OpenCvEncodeFailed)?;
    Ok(buf.to_vec())
}

#[allow(unused)]
pub async fn process_image(
    image_data: Vec<u8>,
    app_state: web::Data<AppState>,
) -> actix_web::Result<Vec<u8>> {
    info!("Recieved image processing request, image size: {} bytes", image_data.len());
    let request_id = Uuid::new_v4();

    // 创建一个tokio的oneshot通道来接收处理后的图片结果
    let (response_tx, response_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();

    {
        let mut pending_requests = app_state.pending_requests.lock().unwrap();
        pending_requests.insert(request_id, response_tx);
        info!("Request(id = `{}`) has been added to pending requests queue, total pending requests: {}", request_id, pending_requests.len());
    }

    let processor_addr: Option<Addr<DataProcessorWs>> = {
        let processors_guard = app_state.data_processors.lock().unwrap();
        let mut next_idx_guard = app_state.next_processor_idx.lock().unwrap();

        if processors_guard.is_empty() {
            error!("No available data processors to handle the image request");
            {
                let mut pending_requests = app_state.pending_requests.lock().unwrap();
                pending_requests.remove(&request_id);
            }
            return Err(Error::NoAvailableProcessors.into());
        }

        let idx = *next_idx_guard % processors_guard.len();
        let selected_processor = processors_guard.get(idx).cloned();
        *next_idx_guard = (*next_idx_guard + 1) % processors_guard.len();
        selected_processor
    };

    let processor_addr = match processor_addr {
        Some(addr) => addr,
        None => {
            error!("No available data processors found");
            {
                let mut pending_requests = app_state.pending_requests.lock().unwrap();
                pending_requests.remove(&request_id);
            }
            return Err(Error::NoAvailableProcessors.into());
        }
    };

    info!("Sending image request with ID: {} to data processor", request_id);
    if let Err(e) = processor_addr.send(ProcessImageForWs(request_id, image_data)).await {
        error!("Failed to send image request to data processor: {:?}", e);
        {
            let mut pending_requests = app_state.pending_requests.lock().unwrap();
            pending_requests.remove(&request_id);
        }
        return Err(Error::ImageTransferError.into());
    }

    match tokio::time::timeout(Duration::from_secs(60), response_rx).await {
        Ok(Ok(processed_image)) => {
            info!("Received processed image data for request ID: {}, size: {} bytes", request_id, processed_image.len());
            Ok(processed_image)
        },
        Ok(Err(e)) => {
            warn!("Failed to receive processed image data for request ID: {}: {:?}", request_id, e);
            Err(Error::ImageTransferError.into())
        },
        Err(_) => {
            error!("Request ID: {} timed out after 60 seconds", request_id);
            {
                let mut pending_requests = app_state.pending_requests.lock().unwrap();
                pending_requests.remove(&request_id);
            }
            Err(Error::Timeout.into())
        }
    }
}

pub async fn ws_connect(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    info!("Websocket connection request received");
    let (res, session, msg_stream) = actix_ws::handle(&req, stream)?;

    DataProcessorWs::create(|ctx| {
        ctx.add_stream(msg_stream);

        DataProcessorWs {
            hb: std::time::Instant::now(),
            session,
            app_state: app_state.clone(),
            self_addr: None,
        }
    });

    info!("Websocket connection established successfully");
    Ok(res)
}

#[get("/image/{id}")]
pub async fn get_image(
    id: web::Path<u32>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse> {
    let id = id.into_inner();
    info!("Received request for image with camera ID: {}", id);

    // 直接用 OpenCV 采集并编码为 JPEG
    let jpeg_data = capture_jpeg(id as i32)?;

    // 处理图片
    let processed_image = process_image(jpeg_data.clone(), app_state).await?;

    Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(processed_image))
}

#[get("/allimage/{id}")]
pub async fn get_cam_image(
    id: web::Path<u32>,
) -> Result<HttpResponse> {
    let id = id.into_inner();
    info!("Received request for image with camera ID: {}", id);

    // 直接用 OpenCV 采集并编码为 JPEG
    let jpeg_data = capture_jpeg(id as i32)?;

    Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(jpeg_data))
}

#[get("/yolov8/{id}")]
pub async fn yolov8_infer(
    id: web::Path<u32>,
) -> Result<HttpResponse> {
    let id = id.into_inner();
    let image_path = format!("//home/cat/data-forward-server/images/{}.jpg", id);

    let exe_path = "/home/cat/lubancat_ai_manual_code/example/yolov8/yolov8_seg/cpp/install/rk3588_linux/rknn_yolov8_seg_demo";
    let model_path = "/home/cat/lubancat_ai_manual_code/example/yolov8/yolov8_seg/cpp/install/rk3588_linux/model/yolov8n-seg.rknn";

    let output = Command::new(exe_path)
        .arg(model_path)
        .arg(&image_path)
        .output()
        .map_err(actix_web::error::ErrorInternalServerError)?;

    if !output.status.success() {
        return Ok(HttpResponse::InternalServerError()
            .body(format!("yolov8 seg demo failed: {:?}", output)));
    }

    let out_img_path = "./out.jpg";
    if !Path::new(out_img_path).exists() {
        return Ok(HttpResponse::InternalServerError()
            .body("output image not found"));
    }
    let img_bytes = fs::read(out_img_path)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(img_bytes))
}

impl ResponseError for Error {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().body(self.to_string())
    }
}