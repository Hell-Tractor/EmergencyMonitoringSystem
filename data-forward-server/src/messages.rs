use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// WebSocket通信中使用的图片消息类型
///
/// 这个枚举定义了两种可能的消息类型：
/// - `Request`: 从分发服务器发送到数据处理服务器的图片处理请求。
/// - `Response`: 从数据处理服务器发送回分发服务器的处理结果。
#[derive(Debug, Serialize, Deserialize)]
pub enum ImageMessage {
    Request(ImageRequest),
    Response(ImageResponse),
}

/// 图片处理请求结构体
///
/// 包含一个唯一的请求ID (`request_id`) 和原始图片数据 (`image_data`)。
/// `request_id` 用于将响应与原始请求关联起来
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageRequest {
    pub request_id: Uuid,
    #[serde(with = "serde_bytes")]
    pub image_data: Vec<u8>,
}

/// 图片处理响应结构体
///
/// 包含原始请求ID (`request_id`) 和处理后的图片数据 (`processed_image_data`)。
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageResponse {
    pub request_id: Uuid,
    #[serde(with = "serde_bytes")]
    pub processed_image_data: Vec<u8>,
}

