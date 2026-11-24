// Máy chủ dịch cục bộ: llama-server (llama.cpp, C++) nạp model Riva-Translate-4B-Instruct-v2 lượng tử 4-bit (GGUF Q4_K_M).
// App gọi API tương thích OpenAI tại 127.0.0.1:PORT. Máy chủ được bật khi cần và tắt khi app thoát.
use crate::config::exe_dir;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub const PORT: u16 = 47870;
/// Mã model trong danh sách của app (tiền tố "local/" để không gửi lên API đám mây)
pub const MODEL_ID: &str = "local/riva-translate-4b";
const MODEL_FILE: &str = "models/riva/Riva-Translate-4B-Instruct-v2-Q4_K_M.gguf";
const SERVER_EXE: &str = "runtime/llama/llama-server.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static CHILD: Mutex<Option<Child>> = Mutex::new(None);

/// Thư mục có thể chứa tài nguyên: cạnh exe (bản cài) hoặc thư mục dự án (khi chạy thử)
fn find(rel: &str) -> Option<PathBuf> {
    [exe_dir(), exe_dir().join("..").join("..").join("..")]
        .into_iter()
        .map(|d| d.join(rel))
        .find(|p| p.exists())
}

fn healthy() -> bool {
    ureq::get(&format!("http://127.0.0.1:{PORT}/health")).timeout(Duration::from_millis(400)).call().is_ok()
}

/// Có thể dùng model này: máy chủ đang chạy (vận hành bên ngoài), hoặc đủ file để app tự bật
pub fn available() -> bool {
    healthy() || (find(MODEL_FILE).is_some() && find(SERVER_EXE).is_some())
}

/// Bật máy chủ nếu chưa chạy, chờ đến khi sẵn sàng nhận yêu cầu
pub fn ensure() -> Result<(), String> {
    if healthy() {
        return Ok(());
    }
    {
        let mut guard = CHILD.lock().map_err(|e| e.to_string())?;
        // Tiến trình cũ đã thoát thì bật lại
        if let Some(c) = guard.as_mut() {
            if matches!(c.try_wait(), Ok(Some(_))) {
                *guard = None;
            }
        }
        if guard.is_none() {
            let exe = find(SERVER_EXE).ok_or("Không tìm thấy llama-server")?;
            let model = find(MODEL_FILE).ok_or("Chưa có file model Riva-Translate 4B")?;
            let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
            let child = Command::new(&exe)
                .current_dir(exe.parent().unwrap_or(&exe))
                .args(["-m", &model.to_string_lossy(), "--host", "127.0.0.1", "--port", &PORT.to_string(), "-c", "2048", "-t", &threads.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("không bật được máy chủ dịch: {e}"))?;
            *guard = Some(child);
        }
    }
    for _ in 0..300 {
        if healthy() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(400));
    }
    Err("máy chủ dịch cục bộ chưa sẵn sàng".into())
}

/// Tắt máy chủ (gọi khi app thoát)
pub fn stop() {
    if let Ok(mut guard) = CHILD.lock() {
        if let Some(mut c) = guard.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
