// Cấu hình người dùng (khớp tên trường với giao diện) và đường dẫn tài nguyên.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "auto" = tự chọn model nhanh nhất; hoặc mã model cố định
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub target_lang: String,
    /// "f5" = giọng AI (Supertonic), "auto" = giọng của máy
    pub voice_name: String,
    pub rate: f32,
    pub auto_speak: bool,
    /// Ngôn ngữ giao diện: "vi", "en" hoặc "ru"
    pub ui_lang: String,
    /// Danh sách model dịch được (lưu sau khi quét)
    pub models: Vec<String>,
    /// 5 model phản hồi nhanh nhất, nhanh nhất đứng đầu
    pub fast_models: Vec<String>,
    /// Thời điểm ping gần nhất (giây kể từ 1970)
    pub last_ping: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "auto".into(),
            base_url: read_env().get("HINDSIGHT_API_LLM_BASE_URL").cloned().unwrap_or_else(|| "https://integrate.api.nvidia.com/v1".into()),
            api_key: String::new(),
            target_lang: "vi".into(),
            voice_name: "f5".into(),
            rate: 1.0,
            auto_speak: false,
            ui_lang: "vi".into(),
            models: Vec::new(),
            fast_models: Vec::new(),
            last_ping: 0,
        }
    }
}

impl Settings {
    /// Khóa API: nhập trong cài đặt, nếu trống thì lấy từ .env
    pub fn effective_key(&self) -> String {
        if !self.api_key.trim().is_empty() {
            return self.api_key.trim().to_string();
        }
        read_env().get("HINDSIGHT_API_LLM_API_KEY").cloned().unwrap_or_default()
    }
}

/// Model mặc định trong .env, dùng khi chưa có danh sách model
pub fn env_model() -> String {
    read_env().get("HINDSIGHT_API_LLM_MODEL").cloned().unwrap_or_else(|| "nvidia/nemotron-3.5-super-vl-120b-a12b".into())
}

/// Thư mục dữ liệu người dùng (cài đặt)
pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    base.join("translator")
}

/// Thư mục chứa file chạy (tài nguyên đóng gói nằm cạnh exe)
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn load_settings() -> Settings {
    fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    let _ = fs::create_dir_all(data_dir());
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = fs::write(settings_path(), json);
    }
}

/// Đọc .env đóng gói cạnh exe (hoặc thư mục dự án khi chạy thử)
pub fn read_env() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let candidates = [exe_dir().join(".env"), exe_dir().join("..").join("..").join("..").join(".env")];
    let Some(path) = candidates.iter().find(|p| p.exists()) else { return map };
    let Ok(text) = fs::read_to_string(path) else { return map };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    map
}
