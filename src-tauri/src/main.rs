// Dịch nhanh (Tauri v2): backend Rust, giao diện HTML/CSS/JS giữ nguyên trong WebView2.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod google;
mod hotkey;
mod riva;
mod supertonic;
mod translate;

use config::Settings;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Size, WindowEvent};

const POPUP_BASE_H: f64 = 168.0;
const POPUP_MIN_H: f64 = 120.0;
const POPUP_MIN_W: f64 = 240.0;
/// Chiều rộng popup tối đa: 1/4 chiều rộng màn hình (tính từ mép phải)
fn max_popup_w(w: &tauri::WebviewWindow) -> f64 {
    let sf = w.scale_factor().unwrap_or(1.0);
    let mw = w.current_monitor().ok().flatten().map(|m| m.size().width as f64 / sf).unwrap_or(1920.0);
    (mw / 4.0).max(POPUP_MIN_W)
}
const POPUP_TOP: f64 = 72.0;
const POPUP_GAP: f64 = 16.0;

/// Ngôn ngữ hiển thị trong giao diện (mã khớp với languages.js)
const UI_LANGS: &[&str] = &["vi", "en", "zh-CN", "ja", "ko", "fr", "de", "es", "it", "pt", "ru", "th", "id", "hi", "ar"];

static POPUP_SEQ: AtomicU64 = AtomicU64::new(0);
static LAST_TEXT: Mutex<String> = Mutex::new(String::new());
static DRAG: Mutex<Option<Drag>> = Mutex::new(None);
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Trạng thái kéo mép popup (tọa độ logical)
struct Drag {
    edge: String,
    sx: f64,
    sy: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

// ---------- Cài đặt ----------
#[tauri::command]
fn settings_get() -> Settings {
    config::load_settings()
}

#[tauri::command]
fn settings_set(patch: Value) -> Result<Settings, String> {
    let mut cur = serde_json::to_value(config::load_settings()).map_err(|e| e.to_string())?;
    if let (Some(c), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            c.insert(k.clone(), v.clone());
        }
    }
    let s: Settings = serde_json::from_value(cur).map_err(|e| e.to_string())?;
    config::save_settings(&s);
    Ok(s)
}

// ---------- Dịch và đọc ----------
#[tauri::command]
async fn translate(text: String, target: String, source: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        let s = config::load_settings();
        match translate::translate(&text, &target, &source, &s) {
            Ok(t) => {
                prefetch_speech(t.clone(), target.clone());
                json!({"ok": true, "text": t})
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

// ---------- Endpoint Google (tách riêng) ----------
#[tauri::command]
async fn google_translate(text: String, target: String, source: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || match google::translate(&text, &target, &source) {
        Ok(t) => json!({"ok": true, "text": t}),
        Err(e) => json!({"ok": false, "error": e}),
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

/// Đọc thành tiếng bằng Google: trả về MP3 dạng base64
#[tauri::command]
async fn google_tts(text: String, lang: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || match google::tts(&text, &lang) {
        Ok(mp3) => json!({"ok": true, "mp3": base64(&mp3)}),
        Err(e) => json!({"ok": false, "error": e}),
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

/// Chuyển giọng nói thành chữ: `pcm` là PCM 16-bit mono 16 kHz; `langs` như ["vi-VN", "en-US"] (nhiều mã = tự chọn)
#[tauri::command]
async fn google_stt(pcm: Vec<u8>, langs: Vec<String>) -> Value {
    tauri::async_runtime::spawn_blocking(move || match google::stt_best(&pcm, &langs) {
        Ok(t) => json!({"ok": true, "text": t}),
        Err(e) => json!({"ok": false, "error": e}),
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

#[tauri::command]
async fn tts_speak(text: String, lang: String) -> Value {
    tauri::async_runtime::spawn_blocking(move || {
        let key = format!("{}\u{1}{}", lang, text);
        if let Some(hit) = CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(&key).cloned())) {
            return json!({"ok": true, "wav": hit});
        }
        if !supertonic::LANGS.contains(&lang.as_str()) {
            return json!({"ok": false, "error": "not-supported"});
        }
        if !supertonic::is_installed() {
            return json!({"ok": false, "error": "not-installed"});
        }
        match supertonic::synthesize(&text, &lang) {
            Ok(wav) => {
                let b64 = base64(&wav);
                if let Ok(mut c) = CACHE.lock() {
                    let m = c.get_or_insert_with(HashMap::new);
                    if m.len() >= 50 {
                        m.clear();
                    }
                    m.insert(key, b64.clone());
                }
                json!({"ok": true, "wav": b64})
            }
            Err(e) if e == "not-supported" => json!({"ok": false, "error": "not-supported"}),
            Err(e) => json!({"ok": false, "error": e}),
        }
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

/// Trạng thái giọng: true = sẵn có, "unsupported" = model không có ngôn ngữ này
fn voice_status_map() -> Value {
    let installed = supertonic::is_installed();
    let mut out = serde_json::Map::new();
    for lang in UI_LANGS {
        let v = if supertonic::LANGS.contains(lang) { json!(installed) } else { json!("unsupported") };
        out.insert(lang.to_string(), v);
    }
    Value::Object(out)
}

#[tauri::command]
fn voice_status() -> Value {
    voice_status_map()
}

/// Quét danh sách model dùng được để dịch và lưu vào cài đặt
#[tauri::command]
async fn models_scan() -> Value {
    tauri::async_runtime::spawn_blocking(|| {
        let mut s = config::load_settings();
        match translate::scan_models(&s) {
            Ok(list) => {
                s.models = list.clone();
                config::save_settings(&s);
                json!({"ok": true, "models": list})
            }
            Err(e) => json!({"ok": false, "error": e}),
        }
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error": "lỗi nội bộ"}))
}

/// Tổng hợp sẵn giọng cho câu vừa dịch, để bấm loa là phát ngay
fn prefetch_speech(text: String, lang: String) {
    if !supertonic::LANGS.contains(&lang.as_str()) || !supertonic::is_installed() {
        return;
    }
    std::thread::spawn(move || {
        let key = format!("{}\u{1}{}", lang, text);
        if let Ok(wav) = supertonic::synthesize(&text, &lang) {
            let b64 = base64(&wav);
            if let Ok(mut c) = CACHE.lock() {
                let m = c.get_or_insert_with(HashMap::new);
                if m.len() >= 50 {
                    m.clear();
                }
                m.insert(key, b64);
            }
        }
    });
}

// ---------- Cửa sổ ----------
#[tauri::command]
fn win_minimize(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

#[tauri::command]
fn win_close(app: AppHandle) {
    riva::stop();
    app.exit(0);
}

/// Mở trình gỡ cài đặt do bộ cài tạo ra, rồi thoát app để gỡ được các file đang dùng
#[tauri::command]
fn uninstall_app(app: AppHandle) -> Result<(), String> {
    let exe = config::exe_dir().join("uninstall.exe");
    if !exe.exists() {
        return Err("Không tìm thấy trình gỡ cài đặt".into());
    }
    riva::stop();
    std::process::Command::new(&exe).spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn popup_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window("popup") {
        let _ = w.hide();
    }
}

fn max_popup_h(w: &tauri::WebviewWindow, y: f64) -> f64 {
    let sf = w.scale_factor().unwrap_or(1.0);
    let mh = w.current_monitor().ok().flatten().map(|m| m.size().height as f64 / sf).unwrap_or(1080.0);
    (mh - y - POPUP_GAP).max(POPUP_MIN_H)
}

#[tauri::command]
fn popup_fit(app: AppHandle, height: f64) {
    let Some(w) = app.get_webview_window("popup") else { return };
    let (Ok(size), Ok(pos), Ok(sf)) = (w.outer_size(), w.outer_position(), w.scale_factor()) else { return };
    let (lw, ly) = (size.width as f64 / sf, pos.y as f64 / sf);
    let max_h = max_popup_h(&w, ly);
    let h = height.clamp(POPUP_MIN_H, max_h);
    let _ = w.set_size(Size::Logical(LogicalSize { width: lw, height: h }));
}

#[tauri::command]
fn popup_drag_start(app: AppHandle, edge: String, x: f64, y: f64) {
    let Some(w) = app.get_webview_window("popup") else { return };
    let (Ok(size), Ok(pos), Ok(sf)) = (w.outer_size(), w.outer_position(), w.scale_factor()) else { return };
    if let Ok(mut d) = DRAG.lock() {
        *d = Some(Drag {
            edge,
            sx: x,
            sy: y,
            x: pos.x as f64 / sf,
            y: pos.y as f64 / sf,
            w: size.width as f64 / sf,
            h: size.height as f64 / sf,
        });
    }
}

#[tauri::command]
fn popup_drag_move(app: AppHandle, x: f64, y: f64) {
    let Some(w) = app.get_webview_window("popup") else { return };
    let Ok(guard) = DRAG.lock() else { return };
    let Some(d) = guard.as_ref() else { return };
    let (dx, dy) = (x - d.sx, y - d.sy);
    let (mut nx, mut nw, mut nh) = (d.x, d.w, d.h);
    if d.edge.contains('l') {
        nw = (d.w - dx).clamp(POPUP_MIN_W, max_popup_w(&w));
        nx = d.x + (d.w - nw);
    }
    if d.edge.contains('b') {
        nh = (d.h + dy).clamp(POPUP_MIN_H, max_popup_h(&w, d.y));
    }
    let _ = w.set_size(Size::Logical(LogicalSize { width: nw, height: nh }));
    let _ = w.set_position(Position::Logical(LogicalPosition { x: nx, y: d.y }));
}

#[tauri::command]
fn popup_drag_end() {
    if let Ok(mut d) = DRAG.lock() {
        *d = None;
    }
}

/// Hiện popup gần góc trên bên phải màn hình
fn show_popup(app: &AppHandle) {
    let Some(w) = app.get_webview_window("popup") else { return };
    let sf = w.scale_factor().unwrap_or(1.0);
    let mw = w.primary_monitor().ok().flatten().map(|m| m.size().width as f64 / sf).unwrap_or(1920.0);
    let pw = w.outer_size().map(|s| s.width as f64 / sf).unwrap_or(300.0).min(max_popup_w(&w));
    let _ = w.set_size(Size::Logical(LogicalSize { width: pw, height: POPUP_BASE_H }));
    let _ = w.set_position(Position::Logical(LogicalPosition { x: mw - pw - 48.0, y: POPUP_TOP }));
    let _ = w.show();
    // Lấy tiêu điểm để bấm ra ngoài popup sẽ làm nó ẩn (xem on_window_event)
    let _ = w.set_focus();
}

#[tauri::command]
fn popup_retranslate(app: AppHandle, target: String) {
    let mut s = config::load_settings();
    s.target_lang = target.clone();
    config::save_settings(&s);
    let text = LAST_TEXT.lock().map(|t| t.clone()).unwrap_or_default();
    if !text.is_empty() {
        let seq = POPUP_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        translate_popup(app, text, target, seq);
    }
}

// ---------- Phím Alt: lấy chữ đang chọn, dịch, hiện popup ----------
fn emit_popup(app: &AppHandle, payload: Value) {
    let _ = app.emit_to("popup", "popup:state", payload);
}

fn translate_popup(app: AppHandle, text: String, target: String, seq: u64) {
    if let Ok(mut t) = LAST_TEXT.lock() {
        *t = text.clone();
    }
    emit_popup(&app, json!({"state": "loading", "text": text, "target": target}));
    std::thread::spawn(move || {
        let s = config::load_settings();
        let result = translate::translate(&text, &target, "auto", &s);
        if POPUP_SEQ.load(Ordering::SeqCst) != seq {
            return;
        }
        let payload = match result {
            Ok(r) => {
                prefetch_speech(r.clone(), target.clone());
                json!({"state": "done", "text": text, "result": r, "target": target})
            }
            Err(e) => json!({"state": "error", "text": text, "error": e, "target": target}),
        };
        emit_popup(&app, payload);
    });
}

fn on_alt(app: AppHandle) {
    std::thread::spawn(move || {
        let Some(text) = hotkey::grab_selection() else { return };
        let text: String = text.chars().take(5000).collect();
        let seq = POPUP_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
        let target = config::load_settings().target_lang;
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || show_popup(&app2));
        translate_popup(app, text, target, seq);
    });
}

// ---------- Mã hóa base64 (tự viết, không cần crate) ----------
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn place_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    if let Some(m) = w.primary_monitor().ok().flatten() {
        let sf = m.scale_factor();
        let (mw, mh) = (m.size().width as f64 / sf, m.size().height as f64 / sf);
        let _ = w.set_position(Position::Logical(LogicalPosition { x: mw - 360.0 - 16.0, y: mh - 470.0 - 56.0 }));
    }
    let _ = w.show();
}

fn main() {
    tauri::Builder::default()
        // Chỉ một bản chạy: lần mở thứ hai sẽ đưa cửa sổ đang chạy lên trước
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_set,
            translate,
            models_scan,
            tts_speak,
            voice_status,
            win_minimize,
            win_close,
            uninstall_app,
            popup_hide,
            popup_fit,
            popup_drag_start,
            popup_drag_move,
            popup_drag_end,
            popup_retranslate,
            google_translate,
            google_tts,
            google_stt,
        ])
        .on_window_event(|window, event| {
            // Đóng widget là thoát app (popup chỉ ẩn)
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { .. } = event {
                    riva::stop();
                    window.app_handle().exit(0);
                }
            }
            // Bấm ra ngoài popup (mất tiêu điểm) thì ẩn popup
            if window.label() == "popup" {
                if let WindowEvent::Focused(false) = event {
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            place_main(&handle);
            // Lần đầu chạy: quét sẵn danh sách model ở nền
            std::thread::spawn(|| {
                let s = config::load_settings();
                if s.models.is_empty() {
                    if let Ok(list) = translate::scan_models(&s) {
                        let mut s = config::load_settings();
                        s.models = list;
                        config::save_settings(&s);
                    }
                }
            });
            hotkey::start({
                let h = handle.clone();
                move || on_alt(h.clone())
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("lỗi khởi động Dịch nhanh")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                riva::stop();
            }
        });
}
