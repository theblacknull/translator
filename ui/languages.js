// Danh sách ngôn ngữ dùng chung cho tiến trình chính (main) và giao diện (renderer).
(function (root) {
  const LANGS = [
    { code: 'vi', vi: 'Tiếng Việt', en: 'Vietnamese', ru: 'Вьетнамский', bcp: 'vi-VN' },
    { code: 'en', vi: 'Tiếng Anh', en: 'English', ru: 'Английский', bcp: 'en-US' },
    { code: 'zh-CN', vi: 'Tiếng Trung (giản thể)', en: 'Simplified Chinese', ru: 'Китайский (упрощённый)', bcp: 'zh-CN' },
    { code: 'zh-TW', vi: 'Tiếng Trung (phồn thể)', en: 'Traditional Chinese', ru: 'Китайский (традиционный)', bcp: 'zh-TW' },
    { code: 'ja', vi: 'Tiếng Nhật', en: 'Japanese', ru: 'Японский', bcp: 'ja-JP' },
    { code: 'ko', vi: 'Tiếng Hàn', en: 'Korean', ru: 'Корейский', bcp: 'ko-KR' },
    { code: 'fr', vi: 'Tiếng Pháp', en: 'French', ru: 'Французский', bcp: 'fr-FR' },
    { code: 'de', vi: 'Tiếng Đức', en: 'German', ru: 'Немецкий', bcp: 'de-DE' },
    { code: 'es', vi: 'Tiếng Tây Ban Nha', en: 'Spanish', ru: 'Испанский', bcp: 'es-ES' },
    { code: 'it', vi: 'Tiếng Ý', en: 'Italian', ru: 'Итальянский', bcp: 'it-IT' },
    { code: 'pt', vi: 'Tiếng Bồ Đào Nha', en: 'Portuguese', ru: 'Португальский', bcp: 'pt-PT' },
    { code: 'ru', vi: 'Tiếng Nga', en: 'Russian', ru: 'Русский', bcp: 'ru-RU' },
    { code: 'th', vi: 'Tiếng Thái', en: 'Thai', ru: 'Тайский', bcp: 'th-TH' },
    { code: 'id', vi: 'Tiếng Indonesia', en: 'Indonesian', ru: 'Индонезийский', bcp: 'id-ID' },
    { code: 'hi', vi: 'Tiếng Hindi', en: 'Hindi', ru: 'Хинди', bcp: 'hi-IN' },
    { code: 'ar', vi: 'Tiếng Ả Rập', en: 'Arabic', ru: 'Арабский', bcp: 'ar-SA' },
  ];

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = { LANGS };
  } else {
    root.LANGS = LANGS;
  }
})(typeof window !== 'undefined' ? window : globalThis);
