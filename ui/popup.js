const $ = (id) => document.getElementById(id);
const LANGS = window.LANGS;
const { setLang, applyI18n, langName: uiLangName } = window.I18N;
let uiLang = null;

// Theo ngôn ngữ giao diện trong Cài đặt: đổi chữ và tên ngôn ngữ trong danh sách
async function refreshLang() {
  try {
    const s = await window.api.getSettings();
    if (s.uiLang === uiLang) return;
    uiLang = s.uiLang;
    setLang(uiLang);
    const keep = els.lang.value;
    els.lang.innerHTML = LANGS.map((l) => `<option value="${l.code}">→ ${uiLangName(l)}</option>`).join('');
    els.lang.value = keep;
    DropdownKit.refresh(els.lang);
  } catch {}
}

const els = {
  card: $('card'),
  head: document.querySelector('.p-head'),
  body: document.querySelector('.p-body'),
  foot: document.querySelector('.p-foot'),
  source: $('p-source'),
  loading: $('p-loading'),
  result: $('p-result'),
  error: $('p-error'),
  lang: $('p-lang'),
  speak: $('p-speak'),
  copy: $('p-copy'),
  speakSrc: $('p-speak-src'),
  close: $('p-close'),
};

let current = null; // { result, target }
let hideTimer = null;
let finished = false; // đã có kết quả (hoặc lỗi) thì mới được tự ẩn
const AUTO_HIDE_MS = 60000;

// Chỉ đếm giờ ẩn khi đã hiện kết quả/lỗi; đang dịch thì không tự ẩn
function scheduleHide() {
  clearTimeout(hideTimer);
  if (!finished) return;
  hideTimer = setTimeout(() => window.api.hidePopup(), AUTO_HIDE_MS);
}

// Danh sách ngôn ngữ đích (chọn nhanh ngay trong popup)
els.lang.innerHTML = LANGS.map((l) => `<option value="${l.code}">→ ${l.vi}</option>`).join('');
DropdownKit.enhance(els.lang);
refreshLang();
els.lang.addEventListener('change', () => {
  current = null;
  els.speak.disabled = true;
  els.copy.disabled = true;
  window.TTS.stop();
  window.api.retranslate(els.lang.value);
});

// Báo chiều cao cần thiết cho main để popup tự co giãn theo nội dung
function fitToContent() {
  const needed =
    els.head.offsetHeight + 3 + els.body.scrollHeight + els.foot.offsetHeight + 2;
  window.api.fit(needed);
}

// Tay nắm đổi kích thước
document.querySelectorAll('.rz').forEach((el) => {
  const edge = el.dataset.edge;
  el.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    el.setPointerCapture(e.pointerId);
    window.api.dragStart(edge, e.screenX, e.screenY);
  });
  el.addEventListener('pointermove', (e) => {
    if (el.hasPointerCapture(e.pointerId)) window.api.dragMove(e.screenX, e.screenY);
  });
  const end = (e) => {
    if (el.hasPointerCapture(e.pointerId)) {
      el.releasePointerCapture(e.pointerId);
      window.api.dragEnd();
    }
  };
  el.addEventListener('pointerup', end);
  el.addEventListener('pointercancel', end);
});

function render(data) {
  refreshLang();
  els.source.textContent = data.text || '';
  els.speakSrc.disabled = !data.text;
  els.error.hidden = true;
  els.result.textContent = '';
  els.loading.hidden = data.state !== 'loading';
  els.speak.disabled = true;
  els.copy.disabled = true;
  current = null;
  finished = data.state === 'done' || data.state === 'error';
  if (data.target) els.lang.value = data.target;

  if (data.state === 'done') {
    current = { result: data.result, target: data.target };
    prefetchF5(data.result, data.target); // tổng hợp sẵn giọng F5 nếu cần
    els.result.textContent = data.result;
    els.speak.disabled = false;
    els.copy.disabled = false;
    maybeAutoSpeak();
  } else if (data.state === 'error') {
    els.error.hidden = false;
    els.error.textContent = data.error;
  }
  requestAnimationFrame(fitToContent);
  scheduleHide();
}

async function prefetchF5(text, lang) {
  const s = await window.api.getSettings();
  if (lang && s.voiceName === 'f5' && text) window.api.ttsSpeak(text, lang).catch(() => {});
}

async function maybeAutoSpeak() {
  const s = await window.api.getSettings();
  if (s.autoSpeak && current) speakCurrent(s);
}

async function speakCurrent(s) {
  const settings = s || (await window.api.getSettings());
  if (!current) return;
  window.TTS.speak(current.result, current.target, { voiceName: settings.voiceName, rate: settings.rate });
}

els.speak.addEventListener('click', () => speakCurrent());

els.copy.addEventListener('click', async () => {
  if (!current) return;
  try {
    await navigator.clipboard.writeText(current.result);
    const prev = els.copy.title;
    els.copy.title = 'Đã sao chép';
    setTimeout(() => { els.copy.title = prev; }, 1200);
  } catch {}
});

els.close.addEventListener('click', () => {
  window.TTS.stop();
  window.api.hidePopup();
});

// Đang rê chuột vào thì không tự ẩn
els.card.addEventListener('mouseenter', () => clearTimeout(hideTimer));
els.card.addEventListener('mouseleave', scheduleHide);

window.api.onPopupState(render);

// Đọc văn bản gốc: đoán ngôn ngữ theo ký tự (Nga, Trung, còn lại coi là Anh)
function guessLang(text) {
  if (/[Ѐ-ӿ]/.test(text)) return 'ru';
  if (/[一-鿿]/.test(text)) return 'zh-CN';
  return 'en';
}

els.speakSrc.addEventListener('click', async () => {
  const text = els.source.textContent;
  if (!text) return;
  const s = await window.api.getSettings();
  window.TTS.speak(text, guessLang(text), { voiceName: s.voiceName, rate: s.rate });
});
