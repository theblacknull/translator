// Cầu nối giao diện <-> backend Rust (Tauri v2): thay thế preload của Electron.
(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  window.api = {
    getSettings: () => invoke('settings_get'),
    setSettings: (patch) => invoke('settings_set', { patch }),
    translate: (text, target, source) => invoke('translate', { text, target, source }),
    ttsSpeak: (text, lang) => invoke('tts_speak', { text, lang }),
    // Endpoint Google (không cần khóa): dịch, đọc, nhận dạng giọng nói
    googleTranslate: (text, target, source) => invoke('google_translate', { text, target, source }),
    googleTts: (text, lang) => invoke('google_tts', { text, lang }),
    googleStt: (pcm, langs) => invoke('google_stt', { pcm, langs }),
    voiceStatus: () => invoke('voice_status'),
    scanModels: () => invoke('models_scan'),
    minimize: () => invoke('win_minimize'),
    close: () => invoke('win_close'),
    uninstall: () => invoke('uninstall_app'),
    hidePopup: () => invoke('popup_hide'),
    retranslate: (target) => invoke('popup_retranslate', { target }),
    fit: (height) => invoke('popup_fit', { height }),
    dragStart: (edge, x, y) => invoke('popup_drag_start', { edge, x, y }),
    dragMove: (x, y) => invoke('popup_drag_move', { x, y }),
    dragEnd: () => invoke('popup_drag_end'),
    onPopupState: (cb) => listen('popup:state', (e) => cb(e.payload)),
  };
})();
