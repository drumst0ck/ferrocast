// Interactive wheel demo: tabs, Shift/Meta/Control keys, arrow-key focus and a hub that names the focused segment.
const root = document.querySelector('[data-wheel-demo]');
if (root) {
  const tabs = [...root.querySelectorAll('[role="tab"]')];
  const panels = [...root.querySelectorAll('.wd-panel')];
  const keyMap = { Shift: 'convert', Meta: 'tools', Control: 'actions' };
  let active = 'convert';
  let visible = true;

  function show(id, focusTab) {
    active = id;
    tabs.forEach((t) => {
      const on = t.dataset.wheel === id;
      t.setAttribute('aria-selected', on ? 'true' : 'false');
      t.tabIndex = on ? 0 : -1;
      if (on && focusTab) t.focus({ preventScroll: true });
    });
    panels.forEach((p) => { p.dataset.on = p.dataset.wheel === id ? 'true' : 'false'; });
  }
  root.classList.add('wd-ready');
  show(active);

  tabs.forEach((t, i) => {
    t.addEventListener('click', () => show(t.dataset.wheel));
    t.addEventListener('keydown', (e) => {
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        e.preventDefault();
        const n = (i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length;
        show(tabs[n].dataset.wheel, true);
      }
    });
  });

  if ('IntersectionObserver' in window) {
    new IntersectionObserver((en) => { visible = en[0].isIntersecting; }, { threshold: 0.2 }).observe(root);
  }
  // Physical key taps switch wheels, like holding a key in the app. Ignored while typing elsewhere.
  document.addEventListener('keydown', (e) => {
    const id = keyMap[e.key];
    if (!id || e.repeat || !visible) return;
    const el = e.target;
    if (el && el.matches && el.matches('input, textarea, select, [contenteditable]')) return;
    show(id);
  });

  panels.forEach((p) => {
    const segs = [...p.querySelectorAll('.wd-seg')];
    const sub = p.querySelector('.wd-hub-sub');
    const cap = p.querySelector('.wd-cap');
    const name = (s) => (s ? s.dataset.label : '');
    function describe(s) {
      if (s) {
        sub.textContent = name(s);
        cap.textContent = s.dataset.note ? `${name(s)}: ${s.dataset.note}.` : name(s);
      } else {
        sub.textContent = sub.dataset.default;
        cap.textContent = cap.dataset.default;
      }
    }
    segs.forEach((s, i) => {
      s.addEventListener('mouseenter', () => describe(s));
      s.addEventListener('mouseleave', () => { if (document.activeElement !== s) describe(null); });
      s.addEventListener('focus', () => { segs.forEach((o) => { o.tabIndex = o === s ? 0 : -1; }); describe(s); });
      s.addEventListener('blur', () => describe(null));
      s.addEventListener('keydown', (e) => {
        const d = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
        if (d) {
          e.preventDefault();
          segs[(i + d + segs.length) % segs.length].focus({ preventScroll: true });
        } else if (e.key === 'Home') { e.preventDefault(); segs[0].focus({ preventScroll: true }); }
        else if (e.key === 'End') { e.preventDefault(); segs[segs.length - 1].focus({ preventScroll: true }); }
      });
    });
  });
}
