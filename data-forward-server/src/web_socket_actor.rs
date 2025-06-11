use actix::fut::wrap_future;
use actix::prelude::*;
use actix_web::web;
use actix_ws::{Message, ProtocolError, Session};
use std::time::{Instant, Duration};
use tracing::{info, warn, error, debug};
use crate::messages::{ImageMessage, ImageRequest};
use crate::api::AppState;
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);

pub struct DataProcessorWs {
    pub hb: Instant,
    pub session: Session,
    pub app_state: web::Data<AppState>,
    pub self_addr: Option<Addr<Self>>,
}

impl Actor for DataProcessorWs {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("DataProcessorWs Actor started.");
        let self_addr = ctx.address();
        {
            let mut processors = self.app_state.data_processors.lock().unwrap();
            processors.push(self_addr.clone());
            info!("Image processing server connected. Active processors: {}", processors.len());
        }
        self.self_addr = Some(self_addr);

        self.hb = Instant::now();
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            act.heartbeat(ctx);
        });
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        warn!("Stopping DataProcessorWs Actor.");
        if let Some(self_addr) = &self.self_addr {
            let mut processors = self.app_state.data_processors.lock().unwrap();
            processors.retain(|addr| addr != self_addr); // 移除当前Actor的地址喵~
            warn!("Image processing server disconnected. Active processors: {}", processors.len());
        }
        Running::Stop
    }
}

impl StreamHandler<Result<Message, ProtocolError>> for DataProcessorWs {
    fn handle(&mut self, msg: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(Message::Ping(msg)) => {
                debug!("Received Ping message: {:?}", msg);
                self.hb = Instant::now();
                let mut session = self.session.clone();
                // respond with Pong to keep the connection alive
                ctx.spawn(wrap_future(async move {
                    if let Err(e) = session.pong(&msg).await {
                        error!("Failed to send Pong response: {:?}", e);
                    }
                }));
            },
            Ok(Message::Pong(_)) => {
                debug!("Received Pong message, resetting heartbeat.");
                self.hb = Instant::now();
            },
            Ok(Message::Text(text)) => {
                debug!("Received Text message: {}", text);
                self.hb = Instant::now();
                // 尝试将收到的文本消息反序列化为`ImageMessage`
                match serde_json::from_str::<ImageMessage>(&text) {
                    Ok(ImageMessage::Response(response)) => {
                        info!("Received ImageResponse with request ID: {}", response.request_id);
                        let mut pending_requests = self.app_state.pending_requests.lock().unwrap();
                        // 查找对应的oneshot发送器（之前由HTTP处理程序存储的）并发送处理后的图片数据
                        if let Some(tx) = pending_requests.remove(&response.request_id) {
                            if let Err(e) = tx.send(response.processed_image_data) {
                                error!("Failed to send processed image data for request ID: {}: {:?}", response.request_id, e);
                            }
                        } else {
                            warn!("No pending request found for request ID: {}", response.request_id);
                        }
                    },
                    Ok(ImageMessage::Request(_)) => {
                        // 数据处理服务器不应该向分发服务器发送`ImageRequest`。
                        warn!("Received unexpected ImageRequest message from data processing server: {}", text);
                    },
                    Err(e) => {
                        // 反序列化失败
                        error!("Failed to deserialize received message as ImageMessage: {:?}", e);
                    }
                }
            },
            Ok(Message::Binary(bin)) => {
                warn!("Received unexpected Binary message(bytes = {})", bin.len());
            },
            Ok(Message::Close(reason)) => {
                warn!("WebSocket connection closed: {:?}", reason);
                ctx.stop();
            },
            Err(e) => {
                warn!("WebSocket error: {:?}", e);
                ctx.stop();
            },
            m => warn!("Received unexpected message: {:?}", m),
        }
    }
}

impl DataProcessorWs {
    fn heartbeat(&mut self, ctx: &mut Context<Self>) {
        if Instant::now().duration_since(self.hb) > CLIENT_TIMEOUT {
            warn!("WebSocket connection timed out, stopping actor.");
            ctx.stop();
            return;
        }

        let mut session = self.session.clone();
        ctx.spawn(wrap_future(async move {
            if let Err(e) = session.ping(b"").await {
                error!("Failed to send Ping message: {:?}", e);
            } else {
                debug!("Ping message sent successfully.");
            }
        }));
    }
}

pub struct ProcessImageForWs(pub Uuid, pub Vec<u8>);

impl actix::Message for ProcessImageForWs {
    type Result = ();
}

impl Handler<ProcessImageForWs> for DataProcessorWs {
    type Result = ();

    fn handle(&mut self, msg: ProcessImageForWs, ctx: &mut Self::Context) {
        let req_id = msg.0;
        let image_data = msg.1;

        let image_request = ImageRequest { request_id: req_id, image_data };
        let image_message = ImageMessage::Request(image_request);

        let serialized_message = match serde_json::to_string(&image_message) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize ImageMessage: {:?}", e);
                return;
            }
        };

        let mut session = self.session.clone();
        ctx.spawn(wrap_future(async move {
            if let Err(e) = session.text(serialized_message).await {
                error!("Failed to send ImageRequest with ID: {}: {:?}", req_id, e);
            } else {
                debug!("ImageRequest with ID: {} sent successfully.", req_id);
            }
        }));
    }
}

