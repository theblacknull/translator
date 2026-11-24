const $ = (id) => document.getElementById(id);
const LANGS = window.LANGS;
const { t, setLang, applyI18n, langName: uiLangName } = window.I18N;

const els = {
  src: $('src'),
  tgt: $('tgt'),
  swap: $('swap'),
  input: $('input'),
  count: $('count'),
  paste: $('paste'),
  mic: $('mic'),
  clear: $('clear'),
  speakSrc: $('speak-src'),
  output: $('output'),
  error: $('error'),
  status: $('status'),
  speakOut: $('speak-out'),
  copy: $('copy'),
  viewTranslate: $('view-translate'),
  viewSettings: $('view-settings'),
  btnSettings: $('btn-settings'),
  btnBack: $('btn-back'),
  btnMin: $('btn-min'),
  btnClose: $('btn-close'),
  setModel: $('set-model'),
  setBase: $('set-base'),
  setKey: $('set-key'),
  setTest: $('set-test'),
  setScan: $('set-scan'),
  setTarget: $('set-target'),
  setAutoSpeak: $('set-autospeak'),
  setVoice: $('set-voice'),
  setRate: $('set-rate'),
  rateVal: $('rate-val'),
  setVoiceTest: $('set-voice-test'),
  setUiLang: $('set-ui-lang'),
  voiceList: $('voice-list'),
  sumModel: $('sum-model'),
  sumTarget: $('sum-target'),
  sumVoice: $('sum-voice'),
  sumUiLang: $('sum-ui-lang'),
};

let settings = null;
let reqId = 0;
let debounceTimer = null;
let lastResult = '';

const AUTO = 'auto';
const UI_LANG_NAMES = { vi: 'Tiếng Việt', en: 'English', ru: 'Русский' };

// Tên ngôn ngữ dịch theo ngôn ngữ giao diện hiện tại
const langName = (code) => {
  const l = LANGS.find((x) => x.code === code);
  return l ? uiLangName(l) : code;
};
const langOptionsHtml = () => LANGS.map((l) => `<option value="${l.code}">${escapeHtml(uiLangName(l))}</option>`).join('');

// Hộp thoại xác nhận: resolve true nếu người dùng đồng ý
function confirmDialog(title, message, okLabel, danger = false) {
  return new Promise((resolve) => {
    const modal = $('modal');
    const ok = $('modal-ok');
    const cancel = $('modal-cancel');
    $('modal-title').textContent = title;
    $('modal-msg').textContent = message;
    ok.textContent = okLabel;
    ok.classList.toggle('danger', danger);
    modal.hidden = false;
    cancel.focus();
    const done = (v) => {
      modal.hidden = true;
      ok.onclick = cancel.onclick = null;
      document.removeEventListener('keydown', key);
      resolve(v);
    };
    const key = (e) => { if (e.key === 'Escape') done(false); if (e.key === 'Enter') done(document.activeElement === ok); };
    ok.onclick = () => done(true);
    cancel.onclick = () => done(false);
    document.addEventListener('keydown', key);
  });
}

// Đổi chữ trên nút tạm thời rồi trả lại như cũ
function flash(btn, label, ms = 2200) {
  if (!btn.dataset.label) btn.dataset.label = btn.textContent;
  clearTimeout(btn._flashT);
  btn.textContent = label;
  btn._flashT = setTimeout(() => { btn.textContent = btn.dataset.label; }, ms);
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

function setError(msg) {
  els.error.hidden = !msg;
  els.error.textContent = msg || '';
}

function setBusy(busy) {
  els.status.textContent = busy ? t('translating') : '';
}

function showOutput(text) {
  lastResult = text || '';
  els.output.classList.toggle('placeholder', !lastResult);
  els.output.textContent = lastResult || t('outputPlaceholder');
  els.speakOut.disabled = !lastResult;
  els.copy.disabled = !lastResult;
}

async function runTranslate() {
  const text = els.input.value.trim();
  els.count.textContent = `${els.input.value.length}/5000`;
  const id = ++reqId;

  if (!text) {
    setError('');
    setBusy(false);
    showOutput('');
    return;
  }

  setError('');
  setBusy(true);
  const res = await window.api.translate(text, els.tgt.value, els.src.value);
  if (id !== reqId) return; // đã có yêu cầu mới hơn

  setBusy(false);
  if (res.ok) {
    showOutput(res.text);
  } else {
    showOutput('');
    setError(res.error);
  }
}

function scheduleTranslate(delay = 300) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(runTranslate, delay);
}

function showView(name) {
  const settingsOpen = name === 'settings';
  els.viewTranslate.hidden = settingsOpen;
  els.viewSettings.hidden = !settingsOpen;
  els.btnSettings.classList.toggle('active', settingsOpen);
}

async function saveSettings(patch) {
  settings = await window.api.setSettings(patch);
  updateSummaries();
}

// ---------- Ngôn ngữ giao diện ----------
// Đổi chữ giao diện và làm mới các danh sách có tên ngôn ngữ
function applyUiLanguage(code) {
  setLang(code);
  const keepSrc = els.src.value;
  const keepTgt = els.tgt.value;
  const keepSetTarget = els.setTarget.value;
  els.src.innerHTML = `<option value="auto">${escapeHtml(t('detect'))}</option>${langOptionsHtml()}`;
  els.tgt.innerHTML = langOptionsHtml();
  els.setTarget.innerHTML = langOptionsHtml();
  els.src.value = keepSrc;
  els.tgt.value = keepTgt;
  els.setTarget.value = keepSetTarget;
  fillModelSelect();
  fillVoiceSelect(settings.voiceName);
  renderVoiceList();
  els.setUiLang.value = code;
  updateSummaries();
  if (!els.output.textContent || els.output.classList.contains('placeholder')) {
    els.output.textContent = t('outputPlaceholder');
  }
  els.count.textContent = `${els.input.value.length}/5000`;
}

// ---------- Model ----------
const GOOGLE_MODEL = 'google/translate';
// Tên hiển thị của model: Google có tên riêng, còn lại lấy phần sau dấu "/"
const modelLabel = (m) => (m === GOOGLE_MODEL ? t('googleModel') : m.split('/').pop());

function fillModelSelect() {
  const models = settings.models || [];
  const current = settings.model || AUTO;
  // Google Translate luôn có trong danh sách, kể cả khi chưa quét model
  const all = new Set([GOOGLE_MODEL, ...models]);
  if (current !== AUTO) all.add(current);
  const fast = settings.fastModels || [];
  const autoLabel = fast.length ? `${t('auto')} · ${modelLabel(fast[0])}` : t('auto');
  const opts = [`<option value="${AUTO}">${escapeHtml(autoLabel)}</option>`];
  for (const m of all) {
    opts.push(`<option value="${escapeHtml(m)}">${escapeHtml(modelLabel(m))}</option>`);
  }
  els.setModel.innerHTML = opts.join('');
  els.setModel.value = current;
}

function updateSummaries() {
  const active = settings.model === AUTO ? (settings.fastModels || [])[0] : settings.model;
  els.sumModel.textContent = settings.model === AUTO
    ? (active ? `${t('auto')} · ${modelLabel(active)}` : t('auto'))
    : modelLabel(settings.model);
  els.sumTarget.textContent = langName(settings.targetLang);
  els.sumVoice.textContent = settings.voiceName === 'f5' ? t('voiceAI') : t('voiceMachine');
  els.sumUiLang.textContent = UI_LANG_NAMES[settings.uiLang] || UI_LANG_NAMES.vi;
}

// ---------- Giọng ----------
function fillVoiceSelect(selected) {
  const voices = window.TTS.getVoices();
  const opts = voices.map((v) => `<option value="${escapeHtml(v.name)}">${escapeHtml(v.name)} (${v.lang})</option>`);
  els.setVoice.innerHTML =
    `<option value="f5">${escapeHtml(t('voiceAIName'))}</option>` +
    `<option value="auto">${escapeHtml(t('voiceSystemGroup'))}</option>` +
    `<optgroup label="${escapeHtml(t('voiceSystemGroup'))}">${opts.join('')}</optgroup>`;
  els.setVoice.value = selected || 'f5';
}

const VOICE_LANGS = ['vi', 'en', 'ru', 'de', 'zh-CN'];

async function renderVoiceList(status) {
  const st = status || (await window.api.voiceStatus());
  els.voiceList.innerHTML = VOICE_LANGS.map((code) => {
    const v = st[code];
    const ai = v && v !== 'unsupported';
    const badge = ai
      ? `<span class="badge on">${escapeHtml(t('badgeAI'))}</span>`
      : `<span class="badge off">${escapeHtml(t('badgeMachine'))}</span>`;
    const name = LANGS.find((l) => l.code === code);
    return `<li class="row"><span class="row-label">${escapeHtml(name ? uiLangName(name) : code)}</span>${badge}</li>`;
  }).join('');
}

function applySettingsToUI() {
  fillModelSelect();
  els.setBase.value = settings.baseUrl || '';
  els.setKey.value = settings.apiKey || '';
  els.setKey.placeholder = settings.apiKey ? '' : t('keyPh');
  els.setTarget.value = settings.targetLang;
  els.setRate.value = settings.rate;
  els.rateVal.textContent = Number(settings.rate).toFixed(1) + '×';
  els.setAutoSpeak.checked = !!settings.autoSpeak;
  fillVoiceSelect(settings.voiceName);
  els.tgt.value = settings.targetLang;
  els.setUiLang.value = settings.uiLang || 'vi';
  updateSummaries();
}

// ---------- Nhập bằng giọng nói ----------
// Ghi âm, cắt đoạn khi có khoảng lặng, gửi lần lượt lên Google STT (dùng được với mọi giọng).
// Không dùng nhận dạng của trình duyệt: thử trong WebView2 cho kết quả rỗng.
let mic = null; // { mode, committed, stopping, queue, ... }
let micBase = ''; // nội dung đã có trong ô nhập trước khi bấm mic

function setMicUi(on) {
  els.mic.classList.toggle('recording', on);
  const label = on ? t('micStop') : t('mic');
  els.mic.title = label;
  els.mic.setAttribute('aria-label', label);
}

// Hiện chữ đã nhận vào ô nhập: nội dung cũ + đoạn đã chốt + đoạn đang nói
function showMic(interim) {
  const parts = [micBase, mic ? mic.committed : '', interim || ''].map((s) => (s || '').trim()).filter(Boolean);
  els.input.value = parts.join(' ').slice(0, 5000);
  els.count.textContent = `${els.input.value.length}/5000`;
}

function commitMic(text) {
  const s = (text || '').trim();
  if (!s) return;
  mic.committed = mic.committed ? `${mic.committed} ${s}` : s;
}

// Đổi mảng mẫu về 16 kHz (tuyến tính) rồi thành PCM 16-bit little-endian
function toPcm16k(chunks, fromRate) {
  let len = 0;
  for (const c of chunks) len += c.length;
  const all = new Float32Array(len);
  let o = 0;
  for (const c of chunks) { all.set(c, o); o += c.length; }
  const outLen = Math.floor(len * 16000 / fromRate);
  const buf = new ArrayBuffer(outLen * 2);
  const view = new DataView(buf);
  const ratio = fromRate / 16000;
  for (let i = 0; i < outLen; i++) {
    const pos = i * ratio;
    const i0 = Math.floor(pos);
    const i1 = Math.min(i0 + 1, len - 1);
    const frac = pos - i0;
    const s = Math.max(-1, Math.min(1, all[i0] * (1 - frac) + all[i1] * frac));
    view.setInt16(i * 2, s < 0 ? s * 0x8000 : s * 0x7fff, true);
  }
  return new Uint8Array(buf);
}

// Ngôn ngữ gửi cho Google STT: mặc định tiếng Việt; nếu chọn ngôn ngữ nguồn cụ thể thì dùng ngôn ngữ đó
function sttLangs() {
  if (els.src.value !== 'auto') {
    const l = LANGS.find((x) => x.code === els.src.value);
    return [l ? l.bcp : 'vi-VN'];
  }
  return ['vi-VN'];
}

async function startMic() {
  if (mic) return;
  micBase = els.input.value;
  mic = { mode: 'chunk', committed: '', stopping: false, queue: Promise.resolve() };
  setMicUi(true);
  startChunk();
}

// Cắt đoạn theo nhịp nói (kiểu VAD của các app thương mại):
// - theo dõi nền tiếng ồn để tự đặt ngưỡng; chỉ bắt đầu đoạn khi có tiếng đủ dài (chống tiếng click)
// - giữ đệm trước khi nói (pre-roll) để không mất chữ đầu
// - kết thúc đoạn khi im lặng đủ lâu (hết câu); nói quá dài thì cắt ở chỗ nhỏ nhất gần nhất
const VAD = {
  preRollMs: 400,     // đệm trước khi nói
  startBuffers: 2,    // số khung liên tiếp có tiếng mới bắt đầu đoạn
  minVoicedMs: 300,   // đoạn có ít tiếng hơn mức này thì bỏ
  endSilenceMs: 700,  // im lặng đủ lâu => hết câu, gửi đoạn
  breathMs: 300,      // nghỉ ngắn (lấy hơi) khi đoạn đã dài
  breathAfterMs: 6000,
  maxMs: 12000,       // quá dài thì cắt ở chỗ nhỏ nhất
  searchMs: 3000,     // khoảng tìm chỗ cắt nhỏ nhất
};

function vadReset(v) {
  Object.assign(v, { state: 'idle', seg: [], segLen: 0, silentMs: 0, startCount: 0 });
}

// Khung: { data, ms, rms, loud }
function vadPushRing(v, frame, rate) {
  v.ring.push(frame);
  v.ringLen += frame.data.length;
  while (v.ring.length > 1 && v.ringLen - v.ring[0].data.length > (VAD.preRollMs / 1000) * rate) {
    v.ringLen -= v.ring.shift().data.length;
  }
}

// Gửi một đoạn lên Google theo thứ tự (hàng đợi)
function queueSegment(chunks, rate, voicedMs) {
  if (voicedMs < VAD.minVoicedMs) return;
  mic.queue = mic.queue.then(async () => {
    if (!mic) return;
    els.status.textContent = t('micBusy');
    const res = await window.api.googleStt(Array.from(toPcm16k(chunks, rate)), sttLangs());
    els.status.textContent = '';
    if (!mic) return;
    if (res.ok) {
      commitMic(res.text);
      showMic('');
    } else {
      setError(res.error);
    }
  });
}

// Kết thúc đoạn. cutAt: chỉ số khung để cắt (nếu nói quá dài); phần sau tiếp tục là đoạn mới
function vadFinish(v, rate, cutAt) {
  const k = cutAt === undefined ? v.seg.length : cutAt + 1;
  const head = v.seg.slice(0, k);
  const tail = v.seg.slice(k);
  const voiced = head.reduce((n, f) => n + (f.loud ? f.ms : 0), 0);
  queueSegment(head.map((f) => f.data), rate, voiced);
  vadReset(v);
  if (tail.length) {
    v.state = 'speech';
    v.seg = tail;
    v.segLen = tail.reduce((n, f) => n + f.data.length, 0);
    let end = tail.length;
    while (end > 0 && !tail[end - 1].loud) end--;
    v.silentMs = (tail.length - end) * tail[0].ms;
  }
}

function vadProcess(v, frame, rate, thr) {
  const { loud } = frame;
  if (v.state === 'idle') {
    if (!loud) v.floor = v.floor * 0.97 + frame.rms * 0.03; // theo dõi tiếng nền
    vadPushRing(v, frame, rate);
    v.startCount = loud ? v.startCount + 1 : 0;
    if (v.startCount >= VAD.startBuffers) {
      v.state = 'speech';
      v.seg = v.ring;
      v.segLen = v.ringLen;
      v.ring = [];
      v.ringLen = 0;
      v.silentMs = 0;
    }
    return;
  }
  v.seg.push(frame);
  v.segLen += frame.data.length;
  if (loud) v.silentMs = 0; else v.silentMs += frame.ms;
  const segMs = (v.segLen / rate) * 1000;

  if (v.silentMs >= VAD.endSilenceMs) return vadFinish(v, rate);
  if (segMs >= VAD.breathAfterMs && v.silentMs >= VAD.breathMs) return vadFinish(v, rate);
  if (segMs >= VAD.maxMs) {
    // tìm khung nhỏ nhất trong vài giây gần nhất để cắt (tránh cắt giữa chữ)
    const from = Math.max(0, v.seg.length - Math.round(VAD.searchMs / frame.ms));
    let best = from;
    for (let i = from; i < v.seg.length; i++) if (v.seg[i].rms < v.seg[best].rms) best = i;
    return vadFinish(v, rate, best);
  }
}

// Một AudioContext dùng chung, tạo sẵn từ lúc mở app: tạo mới mỗi lần bấm mic mất ~650 ms
// trước khi nhận được âm thanh, nên đầu câu bị mất (chữ "hello" đầu tiên)
let audioCtx = null;
function getAudioCtx() {
  if (!audioCtx) audioCtx = new AudioContext();
  return audioCtx;
}

async function startChunk() {
  mic.mode = 'chunk';
  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  } catch {
    setError(t('micErr'));
    finishMic();
    return;
  }
  const ctx = getAudioCtx();
  await ctx.resume();
  const src = ctx.createMediaStreamSource(stream);
  const proc = ctx.createScriptProcessor(2048, 1, 1);
  const v = { floor: 0.004, ring: [], ringLen: 0 };
  vadReset(v);
  Object.assign(mic, { ctx, stream, src, proc, vad: v });
  proc.onaudioprocess = (e) => {
    if (!mic || mic.stopping) return;
    const data = new Float32Array(e.inputBuffer.getChannelData(0));
    let sum = 0;
    for (const x of data) sum += x * x;
    const rms = Math.sqrt(sum / data.length);
    const ms = (data.length / ctx.sampleRate) * 1000;
    const thr = Math.max(v.floor * 3, 0.008);
    vadProcess(v, { data, ms, rms, loud: rms > thr }, ctx.sampleRate, thr);
  };
  src.connect(proc);
  proc.connect(ctx.destination);
}

async function stopMic() {
  if (!mic || mic.stopping) return;
  mic.stopping = true;
  if (mic.proc) mic.proc.disconnect();
  const v = mic.vad;
  if (v && v.state === 'speech') vadFinish(v, mic.ctx.sampleRate);
  mic.queue.then(() => finishMic());
}

function finishMic() {
  if (!mic) return;
  const m = mic;
  mic = null;
  if (m.src) m.src.disconnect();
  if (m.proc) m.proc.disconnect();
  if (m.stream) m.stream.getTracks().forEach((tr) => tr.stop());
  setMicUi(false);
  els.status.textContent = '';
  if (m.committed || micBase) {
    els.input.value = [micBase, m.committed].map((s) => (s || '').trim()).filter(Boolean).join(' ').slice(0, 5000);
    els.count.textContent = `${els.input.value.length}/5000`;
    runTranslate();
  }
}

// Ngôn ngữ nguồn/đích đang chọn trước lần đổi gần nhất (để đổi chỗ khi trùng)
let lastSrc = 'auto';
let lastTgt = null;

// Đặt ngôn ngữ đích. Nếu đích trùng nguồn thì nguồn nhận ngôn ngữ đích cũ (đổi chỗ).
function setTargetLang(code, fromTarget = false) {
  if (fromTarget && code === els.src.value && code !== 'auto') {
    const prev = lastTgt && lastTgt !== code ? lastTgt : (code === 'en' ? 'vi' : 'en');
    els.src.value = prev;
    lastSrc = prev;
  }
  els.tgt.value = code;
  els.setTarget.value = code;
  lastTgt = code;
  saveSettings({ targetLang: code });
  runTranslate();
}

function bindEvents() {
  els.mic.addEventListener('click', () => (mic ? stopMic() : startMic()));
  // Ô nhập & dịch tự động
  els.input.addEventListener('input', () => scheduleTranslate());
  els.src.addEventListener('change', () => {
    const v = els.src.value;
    if (v !== 'auto' && v === els.tgt.value) {
      // Trùng đích: đích nhận ngôn ngữ nguồn cũ (đổi chỗ), nếu không có thì chọn ngôn ngữ khác
      const fallback = lastSrc !== 'auto' && lastSrc !== v ? lastSrc : (v === 'en' ? 'vi' : 'en');
      setTargetLang(fallback);
    }
    lastSrc = els.src.value;
    runTranslate();
  });
  els.tgt.addEventListener('change', () => setTargetLang(els.tgt.value, true));

  els.swap.addEventListener('click', () => {
    const src = els.src.value;
    const tgt = els.tgt.value;
    const carry = lastResult;
    els.src.value = tgt;
    els.tgt.value = src === 'auto' ? (tgt === 'en' ? 'vi' : 'en') : src;
    if (carry) els.input.value = carry;
    runTranslate();
  });

  els.paste.addEventListener('click', async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        els.input.value = text.slice(0, 5000);
        els.input.focus();
        runTranslate();
      }
    } catch {
      els.count.textContent = t('clipboardFail');
    }
  });

  els.clear.addEventListener('click', () => {
    els.input.value = '';
    els.input.focus();
    runTranslate();
  });

  els.speakSrc.addEventListener('click', () => {
    const text = els.input.value.trim();
    if (text) window.TTS.speak(text, els.src.value === 'auto' ? '' : els.src.value, { voiceName: settings.voiceName, rate: settings.rate });
  });

  els.speakOut.addEventListener('click', () => {
    if (lastResult) window.TTS.speak(lastResult, els.tgt.value, { voiceName: settings.voiceName, rate: settings.rate });
  });

  els.copy.addEventListener('click', async () => {
    if (!lastResult) return;
    try {
      await navigator.clipboard.writeText(lastResult);
      els.status.textContent = t('copied');
      setTimeout(() => { if (els.status.textContent === t('copied')) els.status.textContent = ''; }, 1500);
    } catch {
      els.status.textContent = t('copyFail');
    }
  });

  // Cửa sổ
  els.btnMin.addEventListener('click', () => window.api.minimize());
  els.btnClose.addEventListener('click', async () => {
    if (await confirmDialog(t('closeTitle'), t('closeMsg'), t('close'))) window.api.close();
  });
  $('set-uninstall').addEventListener('click', async () => {
    const ok = await confirmDialog(t('uninstallTitle'), t('uninstallMsg'), t('uninstall'), true);
    if (ok) window.api.uninstall();
  });
  els.btnSettings.addEventListener('click', () => showView(els.viewSettings.hidden ? 'settings' : 'translate'));
  els.btnBack.addEventListener('click', () => showView('translate'));

  // Cài đặt: ngôn ngữ ứng dụng
  els.setUiLang.addEventListener('change', async () => {
    await saveSettings({ uiLang: els.setUiLang.value });
    applyUiLanguage(els.setUiLang.value);
  });

  // Cài đặt: model
  els.setModel.addEventListener('change', () => saveSettings({ model: els.setModel.value }));
  els.setBase.addEventListener('change', () => saveSettings({ baseUrl: els.setBase.value.trim() }));
  els.setKey.addEventListener('change', async () => {
    await saveSettings({ apiKey: els.setKey.value.trim() });
    els.setKey.placeholder = settings.apiKey ? '' : t('keyPh');
  });
  els.setScan.addEventListener('click', async () => {
    flash(els.setScan, t('scanning'), 60000);
    const res = await window.api.scanModels();
    if (res.ok) {
      settings = await window.api.getSettings();
      fillModelSelect();
      updateSummaries();
      flash(els.setScan, `${t('scanned')} · ${res.models.length}`);
    } else {
      flash(els.setScan, t('scanErr'), 3000);
    }
  });
  els.setTest.addEventListener('click', async () => {
    flash(els.setTest, t('testing'), 60000);
    const res = await window.api.translate('Hello', 'vi', 'en');
    flash(els.setTest, res.ok ? t('testOk') : t('testFail'), 3000);
  });

  // Cài đặt: dịch
  els.setTarget.addEventListener('change', () => setTargetLang(els.setTarget.value, true));
  els.setAutoSpeak.addEventListener('change', () => saveSettings({ autoSpeak: els.setAutoSpeak.checked }));

  // Cài đặt: giọng
  els.setVoice.addEventListener('change', () => saveSettings({ voiceName: els.setVoice.value }));
  els.setRate.addEventListener('input', () => {
    els.rateVal.textContent = Number(els.setRate.value).toFixed(1) + '×';
  });
  els.setRate.addEventListener('change', () => saveSettings({ rate: Number(els.setRate.value) }));
  els.setVoiceTest.addEventListener('click', () => {
    window.TTS.speak(t('voiceTestText'), els.setTarget.value, { voiceName: els.setVoice.value, rate: Number(els.setRate.value) });
  });

  // Chỉ một nhóm mở tại một thời điểm cho gọn
  document.querySelectorAll('details.grp').forEach((d) => {
    d.addEventListener('toggle', () => {
      if (d.open) document.querySelectorAll('details.grp').forEach((o) => { if (o !== d) o.open = false; });
    });
  });

  // Phím tắt trong ô nhập: Ctrl+Enter dịch ngay
  els.input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      clearTimeout(debounceTimer);
      runTranslate();
    }
  });
}

async function init() {
  getAudioCtx(); // tạo sẵn để bấm mic là có âm thanh ngay
  settings = await window.api.getSettings();
  setLang(settings.uiLang || 'vi');
  els.src.innerHTML = `<option value="auto">${escapeHtml(t('detect'))}</option>${langOptionsHtml()}`;
  els.tgt.innerHTML = langOptionsHtml();
  els.setTarget.innerHTML = langOptionsHtml();
  applySettingsToUI();
  renderVoiceList();
  ['src', 'tgt', 'set-model', 'set-target', 'set-voice', 'set-ui-lang'].forEach((id) => DropdownKit.enhance($(id)));
  els.src.value = 'auto';
  els.tgt.value = settings.targetLang;
  lastTgt = settings.targetLang;
  showOutput('');

  // Giọng của máy có thể đến muộn: làm mới danh sách khi đã nạp xong
  if (window.speechSynthesis) {
    speechSynthesis.addEventListener('voiceschanged', () => fillVoiceSelect(settings.voiceName));
  }

  bindEvents();
  els.input.focus();
}

init();
