// Record section: click or tap the real editor picture and it zooms toward that point the way auto zoom does.
// 2x, ~450 ms ease, back to the whole picture after 1.2 s without another click. No script: the picture stays static.
(() => {
  const frame = document.querySelector('[data-rec-frame]');
  const img = frame && frame.querySelector('img');
  if (!frame || !img) return;
  const ring = frame.querySelector('[data-rec-ring]');
  const SCALE = 2;
  const IDLE = 1200;
  let timer = 0;

  frame.setAttribute('role', 'button');
  frame.setAttribute('tabindex', '0');
  frame.setAttribute('aria-label', frame.dataset.label || '');
  frame.setAttribute('aria-describedby', 'rec-cap');
  frame.dataset.live = '';

  function zoomTo(x, y) {
    // transform-origin moves to the click. The current transform is first re-expressed around the new origin,
    // so a second click while zoomed (or while returning) slides instead of jumping.
    const cur = new DOMMatrix(getComputedStyle(img).transform);
    const ox = parseFloat(img.style.getPropertyValue('--ox')) || 0;
    const oy = parseFloat(img.style.getPropertyValue('--oy')) || 0;
    const s0 = cur.a;
    const tx = cur.e + (1 - s0) * (ox - x);
    const ty = cur.f + (1 - s0) * (oy - y);
    img.style.transition = 'none';
    img.style.setProperty('--ox', x + 'px');
    img.style.setProperty('--oy', y + 'px');
    img.style.transform = `translate(${tx}px, ${ty}px) scale(${s0})`;
    void img.offsetWidth;
    img.style.transition = '';
    img.style.transform = `translate(0px, 0px) scale(${SCALE})`;
    frame.classList.add('is-zoomed');
    clearTimeout(timer);
    timer = setTimeout(back, IDLE);
  }
  function back() {
    img.style.transform = 'translate(0px, 0px) scale(1)';
    frame.classList.remove('is-zoomed');
  }
  function ping(x, y) {
    if (!ring) return;
    ring.style.left = x + 'px';
    ring.style.top = y + 'px';
    ring.classList.remove('go');
    void ring.offsetWidth;
    ring.classList.add('go');
  }
  frame.addEventListener('click', (e) => {
    const r = frame.getBoundingClientRect();
    const x = Math.min(r.width, Math.max(0, e.clientX - r.left));
    const y = Math.min(r.height, Math.max(0, e.clientY - r.top));
    ping(x, y);
    zoomTo(x, y);
  });
  frame.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const r = frame.getBoundingClientRect();
      ping(r.width / 2, r.height / 2);
      zoomTo(r.width / 2, r.height / 2);
    } else if (e.key === 'Escape') { clearTimeout(timer); back(); }
  });
})();
