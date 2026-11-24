// Giọng đọc Supertonic-3 (Supertone, mở, chạy CPU bằng ONNX Runtime). Port trực tiếp từ SDK Python:
// tiền xử lý văn bản -> dự đoán thời lượng -> mã hóa -> khử nhiễu flow-matching -> vocoder.
use ort::{inputs, session::Session, value::Tensor};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_normalization::UnicodeNormalization;

use crate::config::exe_dir;

/// Ngôn ngữ Supertonic-3 hỗ trợ (mã trùng với giao diện). Tiếng Trung và Thái không có trong model.
pub const LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr", "hu", "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk", "vi",
];

const TOTAL_STEPS: i64 = 8;
const SPEED: f32 = 1.0;
const SILENCE_SEC: f32 = 0.3;
const SAMPLE_RATE: u32 = 44100;
const BASE_CHUNK: usize = 512;
const CHUNK_COMPRESS: usize = 6;
const LATENT_DIM: usize = 24;
const MAX_CHUNK_CHARS: usize = 300;
const MAX_CHUNK_CHARS_KO: usize = 120;
const ABBREVIATIONS: &[&str] = &[
    "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "Ph.D.", "etc.", "e.g.", "i.e.", "vs.", "Inc.", "Ltd.", "Co.", "Corp.", "St.", "Ave.", "Blvd.",
];

pub fn model_dir() -> PathBuf {
    let bundled = exe_dir().join("models").join("supertonic");
    if bundled.exists() {
        return bundled;
    }
    // Khi chạy thử từ thư mục dự án
    exe_dir().join("..").join("..").join("..").join("models").join("supertonic")
}

/// Model đã đủ file để chạy
pub fn is_installed() -> bool {
    let d = model_dir();
    ["onnx/duration_predictor.onnx", "onnx/text_encoder.onnx", "onnx/vector_estimator.onnx", "onnx/vocoder.onnx", "onnx/unicode_indexer.json", "voice_styles/M1.json"]
        .iter()
        .all(|f| d.join(f).exists())
}

struct Engine {
    dp: Session,
    text_enc: Session,
    vector_est: Session,
    vocoder: Session,
    indexer: Vec<i64>,
    style_ttl: (Vec<usize>, Vec<f32>),
    style_dp: (Vec<usize>, Vec<f32>),
    rng: u64,
}

static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();

fn load_session(p: &Path) -> Result<Session, String> {
    Session::builder().and_then(|mut b| b.commit_from_file(p)).map_err(|e| format!("không nạp được {}: {e}", p.display()))
}

fn load_style(path: &Path, key: &str) -> Result<(Vec<usize>, Vec<f32>), String> {
    let v: Value = serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let dims: Vec<usize> = v[key]["dims"].as_array().ok_or("thiếu dims")?.iter().map(|x| x.as_u64().unwrap_or(0) as usize).collect();
    fn flat(v: &Value, out: &mut Vec<f32>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| flat(x, out)),
            other => out.push(other.as_f64().unwrap_or(0.0) as f32),
        }
    }
    let mut data = Vec::new();
    flat(&v[key]["data"], &mut data);
    let expect: usize = dims.iter().product();
    if data.len() != expect {
        return Err(format!("giọng {key}: {} phần tử, cần {expect}", data.len()));
    }
    Ok((dims, data))
}

fn engine() -> Result<&'static Mutex<Engine>, String> {
    if let Some(e) = ENGINE.get() {
        return Ok(e);
    }
    let d = model_dir();
    let indexer: Vec<i64> = serde_json::from_str(&fs::read_to_string(d.join("onnx/unicode_indexer.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let eng = Engine {
        dp: load_session(&d.join("onnx/duration_predictor.onnx"))?,
        text_enc: load_session(&d.join("onnx/text_encoder.onnx"))?,
        vector_est: load_session(&d.join("onnx/vector_estimator.onnx"))?,
        vocoder: load_session(&d.join("onnx/vocoder.onnx"))?,
        indexer,
        style_ttl: load_style(&d.join("voice_styles/M1.json"), "style_ttl")?,
        style_dp: load_style(&d.join("voice_styles/M1.json"), "style_dp")?,
        rng: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1,
    };
    let _ = ENGINE.set(Mutex::new(eng));
    ENGINE.get().ok_or_else(|| "lỗi khởi tạo model".to_string())
}

// ---------- Tiền xử lý văn bản (giống SDK) ----------
fn is_emoji(c: char) -> bool {
    matches!(c as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x1F700..=0x1F7FF | 0x1F800..=0x1F8FF | 0x1F900..=0x1F9FF | 0x1FA00..=0x1FAFF | 0x2600..=0x26FF | 0x2700..=0x27BF | 0x1F1E6..=0x1F1FF)
}

fn preprocess(text: &str, lang: &str) -> String {
    let mut t: String = text.nfkd().collect();
    t = t.chars().filter(|c| !is_emoji(*c)).collect();
    let repl: &[(&str, &str)] = &[
        ("\u{2013}", "-"), ("\u{2011}", "-"), ("\u{2014}", "-"), ("\u{00af}", " "), ("_", " "), ("\u{201c}", "\""), ("\u{201d}", "\""), ("\u{2018}", "'"), ("\u{2019}", "'"), ("\u{00b4}", "'"), ("`", "'"), ("[", " "), ("]", " "), ("|", " "), ("/", " "), ("#", " "), ("\u{2192}", " "), ("\u{2190}", " "),
    ];
    for (a, b) in repl {
        t = t.replace(a, b);
    }
    t = t.chars().filter(|c| !"♥☆♡©\\".contains(*c)).collect();
    t = t.replace('@', " at ").replace("e.g.,", "for example, ").replace("i.e.,", "that is, ");
    for (a, b) in [(" ,", ","), (" .", "."), (" !", "!"), (" ?", "?"), (" ;", ";"), (" :", ":"), (" '", "'")] {
        t = t.replace(a, b);
    }
    // Gộp dấu nháy đôi liên tiếp
    let mut out = String::with_capacity(t.len());
    let mut prev: Option<char> = None;
    for c in t.chars() {
        if matches!(c, '"' | '\'' | '`') && prev == Some(c) {
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    let mut t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let ends_ok = t.chars().last().map(|c| ".!?;:,'\"')]}…。」』】〉》›»".contains(c)).unwrap_or(false);
    if !ends_ok {
        t.push('.');
    }
    format!("<{lang}>{t}</{lang}>")
}

// ---------- Chia đoạn theo câu (giống SDK) ----------
fn split_sentences(p: &str) -> Vec<String> {
    let chars: Vec<char> = p.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        cur.push(chars[i]);
        let is_end = matches!(chars[i], '.' | '!' | '?') && i + 1 < chars.len() && chars[i + 1].is_whitespace();
        if is_end {
            let word: String = cur.split_whitespace().last().unwrap_or("").to_string();
            let is_abbr = ABBREVIATIONS.iter().any(|a| word.ends_with(a)) || (word.len() == 2 && word.ends_with('.') && word.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false));
            if !is_abbr {
                out.push(cur.trim().to_string());
                cur.clear();
                // bỏ khoảng trắng sau dấu câu
                i += 1;
                while i + 1 < chars.len() && chars[i + 1].is_whitespace() {
                    i += 1;
                }
            }
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

fn chunk_text(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    for para in text.trim().split("\n\n").map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let mut cur = String::new();
        for s in split_sentences(para) {
            if cur.chars().count() + s.chars().count() + 1 <= max_len {
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(&s);
            } else {
                if !cur.is_empty() {
                    chunks.push(cur.trim().to_string());
                }
                cur = s;
            }
        }
        if !cur.trim().is_empty() {
            chunks.push(cur.trim().to_string());
        }
    }
    chunks
}

// ---------- Số ngẫu nhiên chuẩn (xorshift + Box-Muller) ----------
fn next_u64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn normal(state: &mut u64) -> f32 {
    let u1 = ((next_u64(state) >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
    let u2 = (next_u64(state) >> 11) as f64 / (1u64 << 53) as f64;
    ((-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()) as f32
}

fn synth_chunk(e: &mut Engine, text: &str) -> Result<Vec<f32>, String> {
    // Chuyển ký tự thành chỉ số
    let ids: Vec<i64> = text
        .chars()
        .map(|c| {
            let idx = e.indexer.get(c as usize).copied().unwrap_or(-1);
            if idx < 0 { Err(format!("ký tự không được hỗ trợ: '{c}'")) } else { Ok(idx) }
        })
        .collect::<Result<_, _>>()?;
    let len = ids.len();
    let ids_t = || Tensor::<i64>::from_array(([1usize, len], ids.clone().into_boxed_slice())).map_err(|e| e.to_string());
    let mask_t = || Tensor::<f32>::from_array(([1usize, 1, len], vec![1.0f32; len].into_boxed_slice())).map_err(|e| e.to_string());
    let sdp_t = Tensor::<f32>::from_array((e.style_dp.0.clone(), e.style_dp.1.clone().into_boxed_slice())).map_err(|x| x.to_string())?;
    let sttl_t = || Tensor::<f32>::from_array((e.style_ttl.0.clone(), e.style_ttl.1.clone().into_boxed_slice())).map_err(|x| x.to_string());

    // 1) Thời lượng
    let dp_out = e.dp.run(inputs!["text_ids" => ids_t()?, "style_dp" => sdp_t, "text_mask" => mask_t()?]).map_err(|x| x.to_string())?;
    let (_, dur) = dp_out[0].try_extract_tensor::<f32>().map_err(|x| x.to_string())?;
    let duration = dur.first().copied().unwrap_or(1.0) / SPEED;

    // 2) Mã hóa văn bản
    let enc_out = e.text_enc.run(inputs!["text_ids" => ids_t()?, "style_ttl" => sttl_t()?, "text_mask" => mask_t()?]).map_err(|x| x.to_string())?;
    let (emb_shape, emb) = enc_out[0].try_extract_tensor::<f32>().map_err(|x| x.to_string())?;
    let emb_dims: Vec<usize> = emb_shape.iter().map(|d| *d as usize).collect();
    let emb_vec = emb.to_vec();

    // 3) Nhiễu ban đầu + mặt nạ theo độ dài
    let wav_len = (duration * SAMPLE_RATE as f32) as usize;
    let chunk_size = BASE_CHUNK * CHUNK_COMPRESS;
    let latent_len = (wav_len + chunk_size - 1) / chunk_size;
    let latent_dim = LATENT_DIM * CHUNK_COMPRESS;
    let mut xt: Vec<f32> = (0..latent_dim * latent_len).map(|_| normal(&mut e.rng)).collect();
    let mask: Vec<f32> = vec![1.0; latent_len];
    for c in 0..latent_dim {
        for t in 0..latent_len {
            xt[c * latent_len + t] *= mask[t];
        }
    }
    let lmask = || Tensor::<f32>::from_array(([1usize, 1, latent_len], mask.clone().into_boxed_slice())).map_err(|x| x.to_string());

    // 4) Khử nhiễu theo từng bước
    for step in 0..TOTAL_STEPS {
        let xt_t = Tensor::<f32>::from_array(([1usize, latent_dim, latent_len], xt.clone().into_boxed_slice())).map_err(|x| x.to_string())?;
        let emb_t = Tensor::<f32>::from_array((emb_dims.clone(), emb_vec.clone().into_boxed_slice())).map_err(|x| x.to_string())?;
        let cur_t = Tensor::<f32>::from_array(([1usize], vec![step as f32].into_boxed_slice())).map_err(|x| x.to_string())?;
        let tot_t = Tensor::<f32>::from_array(([1usize], vec![TOTAL_STEPS as f32].into_boxed_slice())).map_err(|x| x.to_string())?;
        let out = e
            .vector_est
            .run(inputs!["noisy_latent" => xt_t, "text_emb" => emb_t, "style_ttl" => sttl_t()?, "text_mask" => mask_t()?, "latent_mask" => lmask()?, "current_step" => cur_t, "total_step" => tot_t])
            .map_err(|x| x.to_string())?;
        let (_, v) = out[0].try_extract_tensor::<f32>().map_err(|x| x.to_string())?;
        xt = v.to_vec();
    }

    // 5) Vocoder -> sóng âm
    let lat_t = Tensor::<f32>::from_array(([1usize, latent_dim, latent_len], xt.into_boxed_slice())).map_err(|x| x.to_string())?;
    let voc = e.vocoder.run(inputs!["latent" => lat_t]).map_err(|x| x.to_string())?;
    let (_, wav) = voc[0].try_extract_tensor::<f32>().map_err(|x| x.to_string())?;
    Ok(wav.to_vec())
}

/// Tổng hợp văn bản thành WAV 16-bit mono (44.1 kHz). Trả "không-hỗ-trợ" nếu ngôn ngữ không có trong model.
pub fn synthesize(text: &str, lang: &str) -> Result<Vec<u8>, String> {
    if !LANGS.contains(&lang) {
        return Err("not-supported".into());
    }
    let eng = engine()?;
    let mut e = eng.lock().map_err(|x| x.to_string())?;
    let max = if lang == "ko" { MAX_CHUNK_CHARS_KO } else { MAX_CHUNK_CHARS };
    let mut all: Vec<f32> = Vec::new();
    let silence = vec![0.0f32; (SILENCE_SEC * SAMPLE_RATE as f32) as usize];
    for (i, chunk) in chunk_text(text, max).iter().enumerate() {
        if i > 0 {
            all.extend_from_slice(&silence);
        }
        let processed = preprocess(chunk, lang);
        all.extend(synth_chunk(&mut e, &processed)?);
    }
    if all.is_empty() {
        return Err("không tạo được âm thanh".into());
    }
    Ok(wav_bytes(&all, SAMPLE_RATE))
}

/// WAV 16-bit mono từ mẫu float
pub fn wav_bytes(samples: &[f32], sr: u32) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sr.to_le_bytes());
    out.extend_from_slice(&(sr * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}
