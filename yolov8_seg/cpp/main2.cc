/*
 * WebSocket client (active connection) with libwebsockets:
 * - Connects to ws://192.168.31.150:8080/ws_connect
 * - Sends local binary jpeg frames (fragmented or full)
 * - Receives processed image (out.jpeg) as binary frames
 *
 * Build: g++ main.cc -o yolov8_client \
 *     -lwebsockets `pkg-config --cflags --libs opencv4` -lrknn_api
 */

#include <libwebsockets.h>
#include <signal.h>
#include <string>
#include <fstream>
#include <vector>
#include <iostream>
#include <thread>
#include <chrono>
#include <opencv2/opencv.hpp>
#include "yolov8_seg.h"
#include "easy_timer.h"
#include <nlohmann/json.hpp> // 需要安装nlohmann/json库

using json = nlohmann::json;

static int interrupted = 0;
static const char* model_path = "/home/cat/lubancat_ai_manual_code/example/yolov8/yolov8_seg/model/yolov8n-seg.rknn";
static rknn_app_context_t rknn_app_ctx;
static struct lws* client_wsi = nullptr;

struct per_session_data {
    std::vector<unsigned char> send_buf;
    std::vector<unsigned char> recv_buf;
    bool recv_binary;
};

struct ImageRequest {
    std::string request_id;
    std::vector<unsigned char> image_data;
};

struct ImageResponse {
    std::string request_id;
    std::vector<unsigned char> processed_image_data;
};

static void sigint_handler(int sig) { interrupted = 1; }

bool init_model() {
    memset(&rknn_app_ctx, 0, sizeof(rknn_app_context_t));
    init_post_process();
    int ret = init_yolov8_seg_model(model_path, &rknn_app_ctx);
    if (ret) {
        std::cerr << "init model failed ret=" << ret << std::endl;
        return false;
    }
    return true;
}

void deinit_model() {
    deinit_post_process();
    release_yolov8_seg_model(&rknn_app_ctx);
}

// Read file into vector
std::vector<unsigned char> load_file(const char* path) {
    std::ifstream f(path, std::ios::binary);
    return std::vector<unsigned char>((std::istreambuf_iterator<char>(f)), std::istreambuf_iterator<char>());
}

bool process_image(const char* in_path, const char* out_path) {
    cv::Mat img = cv::imread(in_path, cv::IMREAD_COLOR);
    if (!img.data) return false;
    cv::Mat rgb;
    cv::cvtColor(img, rgb, cv::COLOR_BGR2RGB);
    cv::Mat resized;
    cv::resize(rgb, resized, cv::Size(640, 640));

    image_buffer_t buf;
    buf.width = 640;
    buf.height = 640;
    buf.format = IMAGE_FORMAT_RGB888;
    buf.virt_addr = resized.data;
    buf.fd = 0;

    object_detect_result_list res;
    if (inference_yolov8_seg_model(&rknn_app_ctx, &buf, &res)) return false;

    unsigned char colors[20][3] = {{255,56,56},{255,157,151},{255,112,31},{255,178,29},{207,210,49},
                                    {72,249,10},{146,204,23},{61,219,134},{26,147,52},{0,212,187},
                                    {44,153,168},{0,194,255},{52,69,147},{100,115,255},{0,24,236},
                                    {132,56,255},{82,0,133},{203,56,255},{255,149,200},{255,55,199}};
    const float alpha = 0.5f;
    float sx = (float)img.cols / 640;
    float sy = (float)img.rows / 640;

    for (int idx = 0; idx < res.count; idx++) {
        auto &seg = res.results_seg[idx];
        uint8_t* mask = seg.seg_mask;
        for (int y = 0; y < 640; y++) {
            for (int x = 0; x < 640; x++) {
                if (!mask[y*640 + x]) continue;
                int ox = std::min((int)(x * sx), img.cols-1);
                int oy = std::min((int)(y * sy), img.rows-1);
                int off = 3*(oy*img.cols + ox);
                int c = mask[y*640 + x] % 20;
                for (int ch = 0; ch < 3; ch++) {
                    float v = colors[c][ch]*(1-alpha) + img.data[off+2-ch]*alpha;
                    img.data[off+2-ch] = (unsigned char)std::min(std::max(v,0.0f),255.0f);
                }
            }
        }
        free(mask);
    }

    char txt[128];
    for (int i = 0; i < res.count; i++) {
        auto& o = res.results[i];
        int x1 = std::min((int)(o.box.left * sx), img.cols-1);
        int y1 = std::min((int)(o.box.top * sy), img.rows-1);
        int x2 = std::min((int)(o.box.right * sx), img.cols-1);
        int y2 = std::min((int)(o.box.bottom * sy), img.rows-1);
        snprintf(txt, sizeof(txt), "%s %.1f%%", coco_cls_to_name(o.cls_id), o.prop*100);
        cv::rectangle(img, cv::Point(x1,y1), cv::Point(x2,y2), cv::Scalar(255,0,0), 2);
        cv::putText(img, txt, cv::Point(x1, y1-5), cv::FONT_HERSHEY_SIMPLEX, 0.6, cv::Scalar(255,255,255),1);
    }

    return cv::imwrite(out_path, img);
}

// 解析ImageMessage::Request
bool parse_image_request(const std::vector<unsigned char>& buf, ImageRequest& req) {
    // 假设前4字节为JSON长度
    if (buf.size() < 4) return false;
    uint32_t json_len = 0;
    memcpy(&json_len, buf.data(), 4);
    if (buf.size() < 4 + json_len) return false;
    std::string json_str((char*)buf.data() + 4, json_len);
    auto j = json::parse(json_str);
    req.request_id = j["Request"]["request_id"];
    req.image_data.assign(buf.begin() + 4 + json_len, buf.end());
    return true;
}

// 构造ImageMessage::Response
std::vector<unsigned char> build_image_response(const std::string& request_id, const std::vector<unsigned char>& img) {
    json j;
    j["Response"]["request_id"] = request_id;
    std::string js = j.dump();
    uint32_t json_len = js.size();
    std::vector<unsigned char> out;
    out.resize(4 + json_len + img.size());
    memcpy(out.data(), &json_len, 4);
    memcpy(out.data() + 4, js.data(), json_len);
    memcpy(out.data() + 4 + json_len, img.data(), img.size());
    return out;
}

static int ws_callback(struct lws* wsi, enum lws_callback_reasons reason,
                       void* user, void* in, size_t len) {
    auto* psd = (per_session_data*)user;
    switch (reason) {
        case LWS_CALLBACK_CLIENT_ESTABLISHED:
            std::cout << "Client connected to server" << std::endl;
            break;
        case LWS_CALLBACK_CLIENT_CONNECTION_ERROR:
            std::cerr << "Client connection error!" << std::endl;
            interrupted = 1;
            break;
        case LWS_CALLBACK_CLIENT_RECEIVE:
            // 累积数据
            psd->recv_buf.insert(psd->recv_buf.end(), (unsigned char*)in, (unsigned char*)in + len);
            if (lws_is_final_fragment(wsi)) {
                ImageRequest req;
                if (!parse_image_request(psd->recv_buf, req)) {
                    std::cerr << "Failed to parse image request" << std::endl;
                    psd->recv_buf.clear();
                    break;
                }
                // 保存JPEG
                std::ofstream of("in_recv.jpg", std::ios::binary);
                of.write((char*)req.image_data.data(), req.image_data.size());
                of.close();

                // 处理图片
                process_image("in_recv.jpg", "out.jpg");

                // 读取处理后的JPEG
                auto processed_img = load_file("out.jpg");

                // 构造响应
                auto resp_buf = build_image_response(req.request_id, processed_img);

                // 发送
                unsigned char* buf = (unsigned char*)malloc(LWS_PRE + resp_buf.size());
                memcpy(buf + LWS_PRE, resp_buf.data(), resp_buf.size());
                lws_write(wsi, buf + LWS_PRE, resp_buf.size(), LWS_WRITE_BINARY);
                free(buf);

                std::cout << "Processed and sent response for request_id: " << req.request_id << std::endl;
                psd->recv_buf.clear();
            }
            break;
        default:
            break;
    }
    return 0;
}

static struct lws_protocols protocols[] = {
    { "default", ws_callback, sizeof(per_session_data), 0 },
    { NULL, NULL, 0, 0 }
};

int main() {
    signal(SIGINT, sigint_handler);
    if (!init_model()) return -1;

    struct lws_context_creation_info info;
    memset(&info, 0, sizeof(info));
    info.port = CONTEXT_PORT_NO_LISTEN;
    info.protocols = protocols;
    auto* ctx = lws_create_context(&info);
    if (!ctx) { std::cerr << "lws init failed" << std::endl; deinit_model(); return -1; }

    struct lws_client_connect_info ccinfo = {0};
    ccinfo.context = ctx;
    ccinfo.address = "192.168.31.150"; // Rust服务器地址
    ccinfo.port = 8080;
    ccinfo.path = "/ws_connect";
    ccinfo.host = ccinfo.address;

    client_wsi = lws_client_connect_via_info(&ccinfo);
    if (!client_wsi) { std::cerr << "Client connect failed" << std::endl; lws_context_destroy(ctx); deinit_model(); return -1; }

    std::cout << "Connecting to ws://192.168.31.150:8080/ws_connect ..." << std::endl;
    while (!interrupted) {
        lws_service(ctx, 100);
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    lws_context_destroy(ctx);
    deinit_model();
    return 0;
}
