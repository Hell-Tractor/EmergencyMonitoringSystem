from flask import Flask, Response, render_template, send_file, request, redirect, url_for, jsonify
import cv2
import threading
import time
import os
import requests

app = Flask(__name__)

# 自动检测可用摄像头，仅保留能正常读取帧的设备
def list_valid_cameras(max_index=10):
    valid = []
    for i in range(max_index):
        dev = f"/dev/video{i}"
        if os.path.exists(dev):
            cap = cv2.VideoCapture(dev, cv2.CAP_V4L2)
            if cap.isOpened():
                ret, frame = cap.read()
                cap.release()
                if ret and frame is not None:
                    valid.append((i, dev))
    return valid

# 获取设备编号和路径列表
camera_devices = list_valid_cameras()
CAMERA_COUNT = len(camera_devices)
camera_snapshots = [None] * CAMERA_COUNT
snapshot_lock = threading.Lock()
active_cam_index = None
current_cam_index = 0

def capture_loop():
    global current_cam_index
    while True:
        if CAMERA_COUNT == 0:
            time.sleep(5)
            continue

        if active_cam_index is not None and current_cam_index == active_cam_index:
            current_cam_index = (current_cam_index + 1) % CAMERA_COUNT
            time.sleep(1)
            continue

        try:
            dev_path = camera_devices[current_cam_index][1]
            cap = cv2.VideoCapture(dev_path, cv2.CAP_V4L2)
            if cap.isOpened():
                ret, frame = cap.read()
                if ret:
                    _, buffer = cv2.imencode('.jpg', frame)
                    image_data = buffer.tobytes()
                    with snapshot_lock:
                        camera_snapshots[current_cam_index] = image_data
                cap.release()
        except Exception as e:
            print(f"Error capturing camera {dev_path}: {e}")

        current_cam_index = (current_cam_index + 1) % CAMERA_COUNT
        time.sleep(2)

def generate_video(cam_index):
    global active_cam_index
    if cam_index != active_cam_index:
        return

    dev_path = camera_devices[cam_index][1]
    cap = cv2.VideoCapture(dev_path, cv2.CAP_V4L2)
    if not cap.isOpened():
        print(f"无法打开摄像头 {dev_path}")
        return

    try:
        while True:
            if active_cam_index != cam_index:
                break
            success, frame = cap.read()
            if not success:
                break
            _, buffer = cv2.imencode('.jpg', frame)
            frame_bytes = buffer.tobytes()
            yield (b'--frame\r\n'
                   b'Content-Type: image/jpeg\r\n\r\n' + frame_bytes + b'\r\n')
            time.sleep(0.05)
    finally:
        cap.release()

threading.Thread(target=capture_loop, daemon=True).start()

@app.route('/')
def index():
    return render_template('index.html', camera_devices=camera_devices)

@app.route('/snapshot/<int:cam_index>')
def snapshot(cam_index):
    with snapshot_lock:
        if 0 <= cam_index < CAMERA_COUNT:
            image_data = camera_snapshots[cam_index]
            if image_data:
                return Response(image_data, mimetype='image/jpeg')
    return send_file('static/loading.gif', mimetype='image/gif')

@app.route('/camera/<int:cam_index>')
def single_camera_view(cam_index):
    global active_cam_index
    active_cam_index = cam_index
    return render_template('camera_view.html', cam_id=cam_index)

@app.route('/video/<int:cam_index>')
def video_feed(cam_index):
    return Response(generate_video(cam_index), mimetype='multipart/x-mixed-replace; boundary=frame')

@app.route('/set_active/<int:cam_index>')
def set_active(cam_index):
    global active_cam_index
    active_cam_index = cam_index
    return "OK"

@app.route('/clear_active')
def clear_active():
    global active_cam_index
    active_cam_index = None
    return "OK"

@app.route('/get_active')
def get_active():
    return str(active_cam_index) if active_cam_index is not None else "none"

INFER_SERVER = "http://100.65.133.179:8080"  # 替换为实际服务器IP
SAVE_PATH = "/home/cat/data-forward-server/images"

@app.route('/inference')
def inference_page():
    return render_template('inference.html', camera_devices=camera_devices)

@app.route('/inference/run/<int:cam_index>')
def run_inference(cam_index):
    global active_cam_index
    active_cam_index = cam_index
    # 检查索引合法性，防止越界
    if cam_index < 0 or cam_index >= len(camera_devices):
        return f"无效的摄像头ID: {cam_index}", 400

    dev_path = camera_devices[cam_index][1]  # 正确获取设备路径
    cap = cv2.VideoCapture(dev_path, cv2.CAP_V4L2)
    ret, frame = cap.read()
    cap.release()
    if not ret:
        return "采集图片失败", 500

    img_path = os.path.join(SAVE_PATH, f"{cam_index}.jpg")
    cv2.imwrite(img_path, frame)

    # 请求推理服务器
    try:
        url = f"{INFER_SERVER}/yolov8/{cam_index}"
        resp = requests.get(url)
        if resp.status_code == 200:
            # 假设返回的是图片
            result_img_path = os.path.join(SAVE_PATH, f"result_{cam_index}.jpg")
            with open(result_img_path, 'wb') as f:
                f.write(resp.content)
            return render_template('inference_result.html', cam_id=cam_index, result_img=f"/show_result/{cam_index}")
        else:
            return f"推理请求失败: {resp.text}", 500
    except Exception as e:
        return f"推理请求异常: {e}", 500

@app.route('/show_result/<int:cam_index>')
def show_result(cam_index):
    result_img_path = os.path.join(SAVE_PATH, f"result_{cam_index}.jpg")
    if os.path.exists(result_img_path):
        return send_file(result_img_path, mimetype='image/jpeg')
    return "结果图片不存在", 404

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)