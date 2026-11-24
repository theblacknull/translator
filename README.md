# Dịch nhanh (Tauri v2)

Widget dịch nhanh cho Windows: bôi đen chữ rồi nhấn **Alt** để dịch và đọc to.

- Backend: Rust (Tauri v2, WebView2 có sẵn trên Windows 10/11)
- Dịch: API tương thích OpenAI (mặc định NVIDIA), khóa lấy từ `.env` hoặc nhập trong Cài đặt
- Giọng đọc: Supertone Supertonic-3 (ONNX Runtime, chạy CPU, đóng gói trong app) cho 31 ngôn ngữ, gồm tiếng Việt. Tiếng Trung và Thái không có trong model nên dùng giọng của máy.

## Cài đặt (người dùng)

Chạy `release/Dich-nhanh_1.0.0_x64-setup.exe`. Chọn ngôn ngữ cài đặt: Tiếng Việt, English, Русский.
Không cần cài Python hay thư viện nào khác.

## Build (nhà phát triển)

Yêu cầu: Node.js, Rust (MSVC), Visual Studio Build Tools (C++), LLVM/libclang. Model nằm ở `models/supertonic/` (tải từ huggingface.co/Supertone/supertonic-3).

```bash
npm install
npx tauri build
```

`src-tauri/ort/` chứa ONNX Runtime 1.24.4 (`onnxruntime.dll`, `onnxruntime.lib`) để link và đóng gói.


## Cấu trúc

| Đường dẫn | Vai trò |
| --- | --- |
| `src-tauri/src/main.rs` | Lệnh Tauri, cửa sổ, popup, luồng phím Alt |
| `src-tauri/src/translate.rs` | Gọi API dịch |
| `src-tauri/src/supertonic.rs` | Giọng Supertonic-3: tiền xử lý, ONNX, vocoder ra WAV |
| `src-tauri/src/hotkey.rs` | Hook phím Alt, Ctrl+C, đọc clipboard (Windows API) |
| `src-tauri/src/config.rs` | Cài đặt người dùng, đọc `.env` |
| `ui/` | Giao diện widget và popup (HTML/CSS/JS) |
| `models/supertonic/` | Model Supertonic-3 và giọng mẫu M1/M2/F1/F2 |

Cài đặt người dùng lưu tại `%APPDATA%\translator\settings.json`.

## Chọn model dịch

Mục **Mô hình AI** trong Cài đặt có danh sách model dịch được (quét từ API). Mặc định là **Tự động**: app ping các model, nhớ 5 model phản hồi nhanh nhất và chỉ ping lại nhóm này sau 30 phút. Có thể chọn một model cố định.

## Máy chủ dịch Riva-Translate 4B (cục bộ)

Model: `models/riva/Riva-Translate-4B-Instruct-v2-Q4_K_M.gguf` (lượng tử 4-bit). Engine: `runtime/llama/llama-server.exe` (llama.cpp).

Chạy máy chủ (để cửa sổ mở trong lúc dùng).

PowerShell:

```powershell
& "C:\Users\DELL\Desktop\translator\runtime\llama\llama-server.exe" -m "C:\Users\DELL\Desktop\translator\models\riva\Riva-Translate-4B-Instruct-v2-Q4_K_M.gguf" --host 127.0.0.1 --port 47870 -c 2048 -t 8
```

cmd:

```bat
"C:\Users\DELL\Desktop\translator\runtime\llama\llama-server.exe" -m "C:\Users\DELL\Desktop\translator\models\riva\Riva-Translate-4B-Instruct-v2-Q4_K_M.gguf" --host 127.0.0.1 --port 47870 -c 2048 -t 8
```

- API tương thích OpenAI: `http://127.0.0.1:47870/v1/chat/completions`
- Kiểm tra máy chủ đã sẵn sàng: `curl http://127.0.0.1:47870/health` (trả về `{"status":"ok"}`)
- Khi máy chủ đang chạy, model `local/riva-translate-4b` xuất hiện trong danh sách model của app. Chọn nó để dịch qua máy chủ này.
- Tham số `-t` là số luồng CPU; giảm xuống nếu máy cần tài nguyên cho việc khác.
