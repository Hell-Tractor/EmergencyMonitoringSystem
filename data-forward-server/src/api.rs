use std::{collections::HashMap, os::windows::process, sync::{Arc, Mutex}, time::Duration};

use actix::{Actor, Addr, AsyncContext};
use actix_web::{get, web, HttpResponse};
use rscam::{Camera, Config};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::web_socket_actor::{DataProcessorWs, ProcessImageForWs};

#[derive(Debug, Default)]
pub struct AppState {
    pub data_processors: Arc<Mutex<Vec<Addr<DataProcessorWs>>>>,
    pub pending_requests: Arc<Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<Vec<u8>>>>>,
    pub next_processor_idx: Arc<Mutex<usize>>,
    pub cameras: Arc<Mutex<HashMap<u32, Camera>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No available data processors found")]
    NoAvailableProcessors,
    #[error("Failed to transfer image data")]
    ImageTransferError,
    #[error("Request timeout")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, Error>;

#[allow(unused)]
pub async fn process_image(
    image_data: Vec<u8>,
    app_state: web::Data<AppState>,
) -> Result<Vec<u8>> {
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
        let processors_guard = app_state.data_processors.lock().unwrap(); // 获取数据处理服务器列表的锁喵~
        let mut next_idx_guard = app_state.next_processor_idx.lock().unwrap(); // 获取轮询索引的锁喵~

        if processors_guard.is_empty() {
            error!("No available data processors to handle the image request");
            {
                let mut pending_requests = app_state.pending_requests.lock().unwrap();
                pending_requests.remove(&request_id);
            }
            return Err(Error::NoAvailableProcessors);
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
            return Err(Error::NoAvailableProcessors);
        }
    };

    info!("Sending image request with ID: {} to data processor", request_id);
    if let Err(e) = processor_addr.send(ProcessImageForWs(request_id, image_data)).await {
        error!("Failed to send image request to data processor: {:?}", e);
        {
            let mut pending_requests = app_state.pending_requests.lock().unwrap();
            pending_requests.remove(&request_id);
        }
        return Err(Error::ImageTransferError);
    }

    match tokio::time::timeout(Duration::from_secs(60), response_rx).await {
        Ok(Ok(processed_image)) => {
            info!("Received processed image data for request ID: {}, size: {} bytes", request_id, processed_image.len());
            Ok(processed_image)
        },
        Ok(Err(e)) => {
            warn!("Failed to receive processed image data for request ID: {}: {:?}", request_id, e);
            Err(Error::ImageTransferError)
        },
        Err(_) => {
            error!("Request ID: {} timed out after 60 seconds", request_id);
            {
                let mut pending_requests = app_state.pending_requests.lock().unwrap();
                pending_requests.remove(&request_id);
            }
            Err(Error::Timeout)
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
pub async fn get_image(id: web::Path<u32>, app_state: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    info!("Received request for image with camera ID: {}", id.into_inner());
    let id = id.into_inner();
    let camera = if let Some(c) = app_state.cameras.lock().unwrap().get(&id) {
        c
    } else {
        let c = Camera::new(&format!("v4l2:///dev/video{}", id))
            .map_err(|e| {
                error!("Failed to open camera {}: {}", id, e);
                actix_web::error::ErrorInternalServerError("Failed to open camera")
            })?;
        c.start(&Config {
            interval: (1, 10),
            resolution: (1920, 1080),
            format: rscam::Format::Jpeg,
            ..Default::default()
        }).unwrap();
        app_state.cameras.lock().unwrap().insert(id, c);
        app_state.cameras.lock().unwrap().get(&id).unwrap()
    };

    let frame = camera.capture().map_err(|e| {
        error!("Failed to capture image from camera {}: {}", id, e);
        actix_web::error::ErrorInternalServerError("Failed to capture image")
    })?;

    info!("Captured image from camera {}: {} bytes", id, frame.len());

    let processed_image = process_image(frame.into(), app_state).await?;

    return Ok(HttpResponse::Ok()
        .content_type("image/jpeg")
        .body(processed_image)
    );
}