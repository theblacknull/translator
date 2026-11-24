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
function fillModelSelect() {
  const models = settings.models || [];
  const current = settings.model || AUTO;
  const all = new Set(models);
  if (current !== AUTO) all.add(current);
  const fast = settings.fastModels || [];
  const autoLabel = fast.length ? `${t('auto')} · ${fast[0].split('/').pop()}` : t('auto');
  const opts = [`<option value="${AUTO}">${escapeHtml(autoLabel)}</option>`];
  for (const m of all) {
    opts.push(`<option value="${escapeHtml(m)}">${escapeHtml(m)}</option>`);
  }
  els.setModel.innerHTML = opts.join('');
  els.setModel.value = current;
}

function updateSummaries() {
  const active = settings.model === AUTO ? (settings.fastModels || [])[0] : settings.model;
  els.sumModel.textContent = settings.model === AUTO
    ? (active ? `${t('auto')} · ${active.split('/').pop()}` : t('auto'))
    : settings.model.split('/').pop();
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

function bindEvents() {
  // Ô nhập & dịch tự động
  els.input.addEventListener('input', () => scheduleTranslate());
  els.src.addEventListener('change', () => runTranslate());
  els.tgt.addEventListener('change', () => {
    saveSettings({ targetLang: els.tgt.value });
    els.setTarget.value = els.tgt.value;
    runTranslate();
  });

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
  els.setTarget.addEventListener('change', () => {
    saveSettings({ targetLang: els.setTarget.value });
    els.tgt.value = els.setTarget.value;
    runTranslate();
  });
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
  showOutput('');

  // Giọng của máy có thể đến muộn: làm mới danh sách khi đã nạp xong
  if (window.speechSynthesis) {
    speechSynthesis.addEventListener('voiceschanged', () => fillVoiceSelect(settings.voiceName));
  }

  bindEvents();
  els.input.focus();
}

init();
