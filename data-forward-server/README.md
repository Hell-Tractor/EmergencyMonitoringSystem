数据转发服务器

## 构建方法

1. 安装[Rust](https://www.rust-lang.org/learn/get-started)
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. 可能需要修改摄像头配置:[api.rs:139,144](data-forward-server/src/api.rs)
3.
   ```
   cd data-forward-server
   cargo run --release
   ```
4. 将监听8080端口，如有需要可在[main.rs](data-forward-server/src/main.rs)中修改
5. 数据处理服务器通过`/ws_connect`与转发服务器建立websocket连接
6. 消息格式参考[message.rs](data-forward-server/src/message.rs)
7. 向`/image/{id}`发送`GET`请求即可获取对应摄像头的图像
1
