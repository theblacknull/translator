// Danh sách chọn tự vẽ (thay thẻ select của hệ điều hành): hiện tối đa 5 mục, còn lại cuộn xem.
// Giữ nguyên <select> ẩn bên dưới nên mọi mã cũ đọc/ghi .value và nghe sự kiện "change" vẫn chạy.
(function () {
  const VISIBLE = 5;
  const ITEM_H = 30;
  let openSel = null;

  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  function place(list, btn) {
    const r = btn.getBoundingClientRect();
    const h = Math.min(list.scrollHeight, VISIBLE * ITEM_H + 8);
    list.style.maxHeight = VISIBLE * ITEM_H + 8 + 'px';
    list.style.left = Math.max(6, r.left) + 'px';
    list.style.width = Math.max(r.width, 160) + 'px';
    const below = r.bottom + 6 + h <= innerHeight - 6;
    list.style.top = (below ? r.bottom + 6 : Math.max(6, r.top - h - 6)) + 'px';
  }

  function close() {
    if (!openSel) return;
    const { list, btn } = openSel._dd;
    list.hidden = true;
    btn.classList.remove('open');
    openSel = null;
  }

  function enhance(sel) {
    if (!sel || sel._dd) return;
    const wrap = document.createElement('div');
    wrap.className = 'dd';
    sel.parentNode.insertBefore(wrap, sel);
    wrap.appendChild(sel);
    sel.classList.add('dd-native');

    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'dd-btn ' + sel.className.replace('dd-native', '').trim();
    btn.setAttribute('aria-haspopup', 'listbox');
    wrap.appendChild(btn);

    const list = document.createElement('div');
    list.className = 'dd-list';
    list.setAttribute('role', 'listbox');
    list.hidden = true;
    document.body.appendChild(list);

    sel._dd = { wrap, btn, list };

    const sync = () => {
      const o = sel.options[sel.selectedIndex];
      btn.textContent = o ? o.text : '';
      btn.disabled = sel.disabled;
      list.innerHTML = Array.from(sel.options)
        .map((opt, i) => `<button type="button" class="dd-item${opt.value === sel.value ? ' on' : ''}" data-i="${i}" role="option">${esc(opt.text)}</button>`)
        .join('');
    };
    sel._dd.sync = sync;

    // Khi mã cũ gán .value hoặc thay danh sách thì cập nhật nút ngay
    const desc = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
    Object.defineProperty(sel, 'value', {
      configurable: true,
      get() { return desc.get.call(this); },
      set(v) { desc.set.call(this, v); sync(); },
    });
    new MutationObserver(sync).observe(sel, { childList: true, subtree: true, attributes: true });

    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      if (openSel === sel) return close();
      close();
      sync();
      // Hiện trước rồi mới đo: danh sách ẩn có chiều cao 0 nên dễ bị đặt sai chỗ và bị cắt
      list.hidden = false;
      place(list, btn);
      btn.classList.add('open');
      openSel = sel;
      const on = list.querySelector('.dd-item.on');
      if (on) on.scrollIntoView({ block: 'nearest' });
    });

    list.addEventListener('click', (e) => {
      const item = e.target.closest('.dd-item');
      if (!item) return;
      const opt = sel.options[Number(item.dataset.i)];
      close();
      if (!opt) return;
      sel.value = opt.value;
      sel.dispatchEvent(new Event('change', { bubbles: true }));
    });

    sync();
  }

  document.addEventListener('click', (e) => {
    if (openSel && !e.target.closest('.dd-list')) close();
  });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
  window.addEventListener('resize', close);
  document.addEventListener('scroll', (e) => {
    const t = e.target;
    if (!(t && t.closest && t.closest('.dd-list'))) close();
  }, true);

  window.DropdownKit = { enhance, refresh: (sel) => sel && sel._dd && sel._dd.sync() };
})();
