// Dịch văn bản qua API tương thích OpenAI (NVIDIA). Quét danh sách model, ping song song
// để chọn model phản hồi nhanh nhất rồi gửi yêu cầu dịch cho model đó.
use crate::config::Settings;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Từ khóa của các model không dùng để dịch (embedding, kiểm duyệt, nhận diện...)
const SKIP: &[&str] = &[
    "embed", "guard", "safety", "detector", "clip", "pii", "calibration", "topic-control", "retriever", "starcoder", "gliner",
];

/// Một agent dùng chung: giữ kết nối TLS để không bắt tay lại mỗi lần
static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

fn agent() -> &'static ureq::Agent {
    AGENT.get_or_init(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build())
}

pub fn language_name(code: &str) -> &'static str {
    match code {
        "vi" => "Vietnamese",
        "en" => "English",
        "zh-CN" => "Simplified Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "th" => "Thai",
        "id" => "Indonesian",
        "hi" => "Hindi",
        "ar" => "Arabic",
        _ => "English",
    }
}

/// Bỏ phần suy luận <think>...</think> nếu mô hình trả về kèm
fn strip_think(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn base(s: &Settings) -> String {
    s.base_url.trim_end_matches('/').to_string()
}

/// Lấy danh sách model từ API và lọc ra model dùng được để dịch
pub fn scan_models(s: &Settings) -> Result<Vec<String>, String> {
    let key = s.effective_key();
    if key.is_empty() {
        return Err("Chưa có API key".into());
    }
    let resp = agent()
        .get(&format!("{}/models", base(s)))
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| e.to_string())?;
    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .ok_or("phản hồi không hợp lệ")?
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .filter(|id| {
            let l = id.to_lowercase();
            !SKIP.iter().any(|k| l.contains(k))
        })
        .collect();
    ids.sort();
    if crate::riva::available() {
        ids.insert(0, crate::riva::MODEL_ID.to_string());
    }
    Ok(ids)
}

/// Thời gian giữa hai lần ping (30 phút)
const PING_INTERVAL_SECS: u64 = 30 * 60;
const TOP_N: usize = 5;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Ping song song các model, trả về danh sách model phản hồi được, nhanh nhất đứng đầu
fn ping(s: &Settings, key: &str, candidates: &[String]) -> Vec<String> {
    let url = format!("{}/chat/completions", base(s));
    let mut results: Vec<(String, u128)> = std::thread::scope(|sc| {
        let handles: Vec<_> = candidates
            .iter()
            .map(|m| {
                let url = url.clone();
                let key = key.to_string();
                let m = m.clone();
                sc.spawn(move || {
                    let t = Instant::now();
                    let body = json!({
                        "model": m,
                        "messages": [{"role": "user", "content": "hi"}],
                        "max_tokens": 1,
                        "temperature": 0,
                        "stream": false,
                        "reasoning_budget": 0,
                        "chat_template_kwargs": {"enable_thinking": false}
                    });
                    let ok = agent()
                        .post(&url)
                        .set("Authorization", &format!("Bearer {key}"))
                        .timeout(Duration::from_secs(6))
                        .send_json(body)
                        .is_ok();
                    (m, t.elapsed().as_millis(), ok)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).filter(|(_, _, ok)| *ok).map(|(m, ms, _)| (m, ms)).collect()
    });
    results.sort_by_key(|(_, ms)| *ms);
    results.into_iter().map(|(m, _)| m).collect()
}


/// Mã ngôn ngữ theo cách model Riva đặt tên trong system prompt
fn riva_code(code: &str) -> &'static str {
    match code {
        "en" => "en",
        "vi" => "vi",
        "zh-CN" => "zh-cn",
        "ja" => "ja",
        "ko" => "ko",
        "fr" => "fr",
        "de" => "de",
        "es" => "es",
        "it" => "it",
        "pt" => "pt-pt",
        "ru" => "ru",
        "th" => "th",
        "id" => "id",
        "hi" => "hi",
        "ar" => "ar",
        _ => "en",
    }
}

/// Đoán ngôn ngữ nguồn khi người dùng chọn "Phát hiện ngôn ngữ"
fn guess_source(text: &str) -> &'static str {
    if text.chars().any(|c| ('\u{1EA0}'..='\u{1EF9}').contains(&c) || c == 'đ' || c == 'Đ') {
        "vi"
    } else if text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) {
        "ru"
    } else {
        "en"
    }
}

/// Gọi model Riva qua llama-server: system prompt = cặp ngôn ngữ (vd. "en-vi"), user = văn bản
fn riva_once(text: &str, pair: &str) -> Result<String, String> {
    let body = json!({
        "messages": [
            {"role": "system", "content": pair},
            {"role": "user", "content": text}
        ],
        "max_tokens": 512,
        "temperature": 0
    });
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", crate::riva::PORT);
    let data: Value = agent()
        .post(&url)
        .timeout(Duration::from_secs(180))
        .send_json(body)
        .map_err(|e| format!("máy chủ Riva lỗi: {e}"))?
        .into_json()
        .map_err(|e| e.to_string())?;
    let out = data["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if out.is_empty() {
        return Err("Riva không trả về nội dung".into());
    }
    Ok(out)
}

/// Dịch bằng model cục bộ. Model chỉ có cặp có tiếng Anh, nên cặp khác đi qua tiếng Anh.
fn translate_local(text: &str, target: &str, source: &str) -> Result<String, String> {
    crate::riva::ensure()?;
    let src = if source == "auto" { guess_source(text) } else { riva_code(source) };
    let tgt = riva_code(target);
    if src == tgt {
        return Ok(text.to_string());
    }
    if src == "en" || tgt == "en" {
        return riva_once(text, &format!("{src}-{tgt}"));
    }
    let english = riva_once(text, &format!("{src}-en"))?;
    riva_once(&english, &format!("en-{tgt}"))
}

/// Chọn model để dịch.
/// - Model cố định: dùng luôn.
/// - Tự động: trong 30 phút dùng model nhanh nhất đã nhớ; hết hạn thì ping 5 model đó (hoặc tất cả nếu chưa có) và nhớ lại 5 model nhanh nhất.
fn choose_model(s: &Settings, key: &str) -> String {
    if s.model != "auto" {
        return s.model.clone();
    }
    let cloud: Vec<String> = s.models.iter().filter(|m| !m.starts_with("local/")).cloned().collect();
    let now = now_secs();
    let fresh = !s.fast_models.is_empty() && now.saturating_sub(s.last_ping) < PING_INTERVAL_SECS;
    if fresh {
        return s.fast_models[0].clone();
    }
    let candidates = if s.fast_models.is_empty() { cloud.clone() } else { s.fast_models.clone() };
    if candidates.is_empty() {
        return crate::config::env_model();
    }
    let mut ranked = ping(s, key, &candidates);
    if ranked.is_empty() && !cloud.is_empty() {
        ranked = ping(s, key, &cloud);
    }
    if ranked.is_empty() {
        return crate::config::env_model();
    }
    ranked.truncate(TOP_N);
    // Lưu lại 5 model nhanh nhất và thời điểm ping
    let mut saved = crate::config::load_settings();
    saved.fast_models = ranked.clone();
    saved.last_ping = now;
    crate::config::save_settings(&saved);
    ranked[0].clone()
}

pub fn translate(text: &str, target: &str, source: &str, s: &Settings) -> Result<String, String> {
    let key = s.effective_key();
    if key.is_empty() {
        return Err("Chưa có API key (nhập trong Cài đặt hoặc file .env)".into());
    }
    let model = choose_model(s, &key);
    if model.starts_with("local/") {
        return translate_local(text, target, source);
    }
    let local = false;
    let (url, auth) = if local {
        crate::riva::ensure()?;
        (format!("http://127.0.0.1:{}/v1/chat/completions", crate::riva::PORT), None)
    } else {
        (format!("{}/chat/completions", base(s)), Some(format!("Bearer {key}")))
    };
    let model = if local { "riva-translate".to_string() } else { model };
    let target_name = language_name(target);
    let source_part = if source == "auto" {
        "Detect the source language automatically.".to_string()
    } else {
        format!("The source language is {}.", language_name(source))
    };
    let system = format!(
        "You are a professional translation engine. Translate the user's text into {target_name}. \
         If the text is already in {target_name}, translate it into English instead. {source_part} \
         Keep the original meaning, tone, line breaks and formatting. \
         Output ONLY the translated text. No explanations, notes, quotes or labels."
    );
    let base_body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": text}
        ],
        "temperature": 0.2,
        "max_tokens": 1024,
        "stream": false,
    });
    // Tắt bước suy nghĩ cho nhanh; nếu máy chủ không hỗ trợ thì gọi lại bình thường
    let mut fast = base_body.clone();
    fast["reasoning_budget"] = json!(0);
    fast["chat_template_kwargs"] = json!({"enable_thinking": false});

    let send = |body: &Value| {
        let req = agent().post(&url);
        let req = match &auth {
            Some(a) => req.set("Authorization", a),
            None => req,
        };
        req.timeout(Duration::from_secs(if local { 120 } else { 60 })).send_json(body.clone())
    };
    let resp = if local {
        send(&base_body)
    } else {
        match send(&fast) {
            Err(ureq::Error::Status(400, _)) => send(&base_body),
            other => other,
        }
    };
    let data: Value = resp
        .map_err(|e| match e {
            ureq::Error::Status(code, r) => format!("Lỗi HTTP {code}: {}", r.into_string().unwrap_or_default()),
            other => other.to_string(),
        })?
        .into_json()
        .map_err(|e| e.to_string())?;
    let content = data["choices"][0]["message"]["content"].as_str().unwrap_or("");
    let out = strip_think(content);
    if out.is_empty() {
        return Err("Mô hình không trả về nội dung dịch".into());
    }
    Ok(out)
}
