mod web_socket_actor;
mod api;
mod messages;

use actix_web::{web, App, HttpServer};
use tracing::info;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

use crate::api::ws_connect;
use tracing_subscriber::fmt::writer::MakeWriterExt;
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let file_appender = rolling::daily("logs", "server.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    std::mem::forget(guard); // Prevent guard from being dropped
    let subscriber = tracing_subscriber::fmt()
        .with_writer(file_writer.and(std::io::stdout))
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("data-forward-server=debug")))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");
    info!("Starting data-forward-server...");
    info!("Logs will be written to ./logs/**");

    HttpServer::new(|| {
        App::new()
            .service(
                web::resource("/ws_connect")
                    .route(web::get().to(ws_connect)) // 注册WebSocket连接接口
            )
            .service(api::get_image)
            .service(api::get_cam_image)
            .service(api::yolov8_infer) // <-- 新增
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
