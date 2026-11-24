// Google Translate không cần khóa API (viết lại từ d4n3436/GTranslate, bản Rust gọn).
// Gồm 3 đầu cuối: dịch văn bản, đọc thành tiếng (TTS) và nhận dạng giọng nói (STT).
// Lưu ý: đây là các endpoint web không chính thức của Google; có thể đổi hoặc ngừng bất cứ lúc nào.
use serde_json::Value;
use std::time::Duration;

/// Mã model trong danh sách của app (không gửi lên API đám mây)
pub const MODEL_ID: &str = "google/translate";
const TTS_URL: &str = "https://translate.google.com/translate_tts";
const STT_URL: &str = "https://www.google.com/speech-api/v2/recognize";
// Khóa công khai dùng bởi chức năng nhận dạng giọng nói của trình duyệt Chromium
const STT_KEY: &str = "AIzaSyBOti4mM-6x9WDnZIjIeyEU21OpBXqWBgw";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

// TLS của hệ điều hành (schannel trên Windows): Google chặn một số kiểu TLS khác (lỗi 429)
fn agent() -> ureq::Agent {
    let tls = native_tls::TlsConnector::new().expect("native TLS");
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .tls_connector(std::sync::Arc::new(tls))
        .build()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// Thuật toán token "tk" của Google Translate (giống bản C# gốc)
fn work(mut num: i64, seed: &[u8]) -> i64 {
    let mut i = 0;
    while i + 2 < seed.len() {
        let mut d = seed[i + 2] as i64;
        if d >= b'a' as i64 {
            d -= b'W' as i64;
        }
        if seed[i + 1] == b'+' {
            num = (num + (num >> d)) & 0xFFFF_FFFF;
        } else {
            num ^= num << d;
        }
        i += 3;
    }
    num
}

fn token(text: &str) -> String {
    let b = now_secs() / 3600;
    let mut a = b;
    for ch in text.encode_utf16() {
        a = work(a + ch as i64, b"+-a^+6");
    }
    a = work(a, b"+-3^+b+-f");
    if a < 0 {
        a = (a & i32::MAX as i64) + i32::MAX as i64 + 1;
    }
    a %= 1_000_000;
    format!("{}.{}", a, a ^ b)
}

/// Dịch văn bản. `source` = "auto" để tự nhận diện ngôn ngữ.
/// Google hay trả 429 khi gọi dồn: thử lần lượt 2 máy chủ, có chờ ngắn giữa các lần.
pub fn translate(text: &str, target: &str, source: &str) -> Result<String, String> {
    let tk = token(text);
    let attempts: [(&str, Vec<(&str, String)>); 2] = [
        (
            "https://translate.google.com/translate_a/single",
            vec![("client", "tw-ob".into()), ("sl", source.into()), ("tl", target.into()), ("dt", "t".into()), ("tk", tk.clone()), ("q", text.into())],
        ),
        (
            "https://translate.googleapis.com/translate_a/single",
            vec![("client", "gtx".into()), ("sl", source.into()), ("tl", target.into()), ("dt", "t".into()), ("dj", "1".into()), ("q", text.into())],
        ),
    ];
    let mut last_err = String::new();
    for round in 0..2 {
        for (url, params) in attempts.iter() {
            let pairs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
            match agent().get(url).set("User-Agent", UA).query_pairs(pairs).call() {
                Ok(resp) => {
                    let v: Value = resp.into_json().map_err(|e| e.to_string())?;
                    return parse_translation(&v);
                }
                Err(e) => last_err = format!("Google Translate lỗi: {e}"),
            }
        }
        if round == 0 {
            std::thread::sleep(Duration::from_millis(1500));
        }
    }
    Err(last_err)
}

// Hai dạng phản hồi: [[["bản dịch","gốc",..],..],..] (tw-ob) hoặc {"sentences":[{"trans":..}]} (dj=1)
fn parse_translation(v: &Value) -> Result<String, String> {
    if let Some(sents) = v["sentences"].as_array() {
        return Ok(sents.iter().filter_map(|p| p["trans"].as_str()).collect());
    }
    let parts = v[0].as_array().ok_or("Google Translate trả về dữ liệu lạ")?;
    Ok(parts.iter().filter_map(|p| p[0].as_str()).collect())
}

/// Chia văn bản thành các đoạn tối đa ~180 ký tự, cắt theo khoảng trắng
fn split_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > 180 {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Đọc thành tiếng: trả về các đoạn MP3 nối tiếp nhau (đọc theo thứ tự)
pub fn tts(text: &str, lang: &str) -> Result<Vec<u8>, String> {
    let chunks = split_text(text);
    let total = chunks.len();
    let mut audio = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let url = format!(
            "{TTS_URL}?ie=UTF-8&tl={lang}&ttsspeed=1&total={total}&idx={idx}&client=tw-ob&textlen={}&tk={}",
            chunk.chars().count(),
            token(chunk)
        );
        let resp = agent()
            .get(&url)
            .set("User-Agent", UA)
            .query("q", chunk)
            .call()
            .map_err(|e| format!("Google TTS lỗi: {e}"))?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf).map_err(|e| e.to_string())?;
        audio.extend(buf);
    }
    Ok(audio)
}

/// Nhận dạng giọng nói với một ngôn ngữ. Trả về (chữ, độ tin cậy 0..1).
pub fn stt(pcm: &[u8], lang: &str) -> Result<(String, f64), String> {
    let url = format!("{STT_URL}?output=json&lang={lang}&key={STT_KEY}&client=chromium");
    let resp = agent()
        .post(&url)
        .set("User-Agent", UA)
        .set("Content-Type", "audio/l16; rate=16000")
        .send_bytes(pcm)
        .map_err(|e| format!("Google STT lỗi: {e}"))?;
    let body = resp.into_string().map_err(|e| e.to_string())?;
    // Phản hồi là nhiều dòng JSON; lấy kết quả đầu tiên có chữ
    for line in body.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let alt = &v["result"][0]["alternative"][0];
        if let Some(t) = alt["transcript"].as_str() {
            let conf = alt["confidence"].as_f64().unwrap_or(0.5);
            return Ok((t.to_string(), conf));
        }
    }
    Ok((String::new(), 0.0))
}

/// Kết quả có đúng "chữ" của ngôn ngữ đó không?
/// Độ tin cậy của Google không so sánh được giữa các ngôn ngữ, nên dùng cách kiểm tra chữ viết:
/// - tiếng Nga: phải có chữ Cyrillic
/// - tiếng Việt: phải có dấu tiếng Việt (ă â đ ê ô ơ ư và dấu thanh)
/// - các ngôn ngữ Latin khác: không được có chữ Cyrillic hay dấu tiếng Việt
fn fits_script(text: &str, lang: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return false;
    }
    let cyr = letters.iter().filter(|c| ('\u{0400}'..='\u{04FF}').contains(*c)).count();
    let viet = letters
        .iter()
        .filter(|c| "ăâđêôơưĂÂĐÊÔƠƯ".contains(**c) || ('\u{1EA0}'..='\u{1EF9}').contains(*c))
        .count();
    let cyr_majority = cyr * 2 > letters.len();
    if lang.starts_with("ru") {
        cyr_majority
    } else if lang.starts_with("vi") {
        !cyr_majority && viet > 0
    } else {
        !cyr_majority && viet == 0
    }
}

/// Nhận dạng với nhiều ngôn ngữ ứng viên (khi người dùng chọn "Phát hiện ngôn ngữ"):
/// gửi song song, giữ kết quả đúng chữ viết của ngôn ngữ, rồi lấy kết quả tin cậy nhất.
pub fn stt_best(pcm: &[u8], langs: &[String]) -> Result<String, String> {
    let results: Vec<(String, Result<(String, f64), String>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = langs
            .iter()
            .map(|l| (l.clone(), sc.spawn(move || stt(pcm, l))))
            .collect();
        handles
            .into_iter()
            .map(|(l, h)| (l, h.join().unwrap_or_else(|_| Err("lỗi nội bộ".into()))))
            .collect()
    });
    let mut best: Option<(f64, String)> = None;
    let mut fallback: Option<String> = None;
    let mut last_err = None;
    for (lang, r) in results {
        match r {
            Ok((text, conf)) if !text.trim().is_empty() => {
                if fallback.is_none() {
                    fallback = Some(text.clone());
                }
                if fits_script(&text, &lang) && best.as_ref().map_or(true, |(c, _)| conf > *c) {
                    best = Some((conf, text));
                }
            }
            Ok(_) => {}
            Err(e) => last_err = Some(e),
        }
    }
    match (best, fallback, last_err) {
        (Some((_, t)), _, _) => Ok(t),
        (None, Some(t), _) => Ok(t),
        (None, None, Some(e)) => Err(e),
        (None, None, None) => Ok(String::new()),
    }
}
