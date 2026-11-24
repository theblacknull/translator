// Đọc văn bản: giọng của máy (Web Speech API) hoặc giọng F5-TTS tiếng Nga (AI, chạy qua main).
// speak(text, langCode, { voiceName, rate, onEnd })  — voiceName = 'auto' | 'f5' | tên giọng máy
(function () {
  let voices = [];
  let audio = null;
  let token = 0; // bỏ qua kết quả F5 đến muộn khi đã có lượt đọc mới

  function refreshVoices() {
    voices = window.speechSynthesis ? speechSynthesis.getVoices() : [];
  }

  if (window.speechSynthesis) {
    refreshVoices();
    speechSynthesis.onvoiceschanged = refreshVoices;
  }

  function getVoices() {
    if (!voices.length) refreshVoices();
    return voices;
  }

  function findVoice(langCode, voiceName) {
    const list = getVoices();
    if (voiceName && voiceName !== 'auto') {
      const chosen = list.find((v) => v.name === voiceName);
      if (chosen) return chosen;
    }
    const meta = (window.LANGS || []).find((l) => l.code === langCode);
    const bcp = (meta ? meta.bcp : langCode || '').toLowerCase();
    if (!bcp) return null;
    const primary = bcp.split('-')[0];
    return (
      list.find((v) => v.lang && v.lang.toLowerCase() === bcp) ||
      list.find((v) => v.lang && v.lang.toLowerCase().startsWith(primary)) ||
      null
    );
  }

  function speakSystem(text, langCode, opts) {
    if (!window.speechSynthesis) return;
    speechSynthesis.cancel();
    const utter = new SpeechSynthesisUtterance(text);
    const voice = findVoice(langCode, opts.voiceName);
    if (voice) {
      utter.voice = voice;
      utter.lang = voice.lang;
    } else {
      const meta = (window.LANGS || []).find((l) => l.code === langCode);
      if (meta) utter.lang = meta.bcp;
    }
    utter.rate = opts.rate || 1;
    utter.onend = () => opts.onEnd && opts.onEnd();
    utter.onerror = () => opts.onEnd && opts.onEnd();
    speechSynthesis.speak(utter);
  }

  async function speakAI(text, langCode, opts) {
    const id = ++token;
    const res = await window.api.ttsSpeak(text, langCode);
    if (id !== token) return; // đã có lượt đọc mới hơn
    if (!res.ok) {
      // Chưa cài giọng hoặc lỗi: đọc bằng giọng máy
      speakSystem(text, langCode, { ...opts, voiceName: 'auto' });
      if (opts.onError) opts.onError(res.error);
      return;
    }
    audio = new Audio('data:audio/wav;base64,' + res.wav);
    audio.playbackRate = opts.rate || 1;
    audio.onended = () => opts.onEnd && opts.onEnd();
    audio.play();
  }

  function speak(text, langCode, opts = {}) {
    if (!text) return;
    stop();
    // Giọng AI (Supertonic-3) cho ngôn ngữ đã biết; ngôn ngữ chưa xác định dùng giọng máy
    if (opts.voiceName === 'f5' && langCode) return speakAI(text, langCode, opts);
    speakSystem(text, langCode, { ...opts, voiceName: opts.voiceName === 'f5' ? 'auto' : opts.voiceName });
  }

  function stop() {
    token++;
    if (audio) {
      audio.pause();
      audio = null;
    }
    if (window.speechSynthesis) speechSynthesis.cancel();
  }

  window.TTS = { speak, stop, getVoices };
})();
