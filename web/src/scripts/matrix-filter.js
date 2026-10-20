// Client-side filter for the conversion explorer. The full list is already in the HTML; this only hides things.
const root = document.querySelector('[data-matrix]');
if (root) {
  const q = root.querySelector('#mx-q');
  const from = root.querySelector('#mx-from');
  const to = root.querySelector('#mx-to');
  const free = root.querySelector('#mx-free');
  const famBtns = [...root.querySelectorAll('.mx-fams button')];
  const count = root.querySelector('#mx-count');
  const empty = root.querySelector('#mx-empty');
  const total = Number(root.dataset.total);
  const fams = [...root.querySelectorAll('.mx-fam')].map((el) => ({
    el,
    id: el.dataset.fam,
    rows: [...el.querySelectorAll('.mx-row')].map((row) => ({ row, from: row.dataset.from, text: row.dataset.text, chips: [...row.querySelectorAll('.mx-chip')] })),
  }));
  let fam = '';

  function apply(writeHash) {
    const text = q.value.trim().toLowerCase();
    let shown = 0;
    for (const f of fams) {
      let famShown = 0;
      const famOk = !fam || fam === f.id;
      for (const r of f.rows) {
        const fromOk = !from.value || from.value === r.from;
        const rowTextHit = !text || r.text.includes(text);
        let n = 0;
        for (const c of r.chips) {
          const ok = famOk && fromOk
            && (!to.value || c.dataset.to === to.value)
            && (!free.checked || !c.dataset.needs)
            && (rowTextHit || c.dataset.text.includes(text));
          c.hidden = !ok;
          if (ok) n++;
        }
        r.row.hidden = n === 0;
        famShown += n;
      }
      f.el.hidden = famShown === 0;
      shown += famShown;
    }
    count.innerHTML = `<b class="nums">${shown}</b> of ${total} conversion pairs shown`;
    empty.hidden = shown !== 0;
    if (writeHash) {
      const h = text ? '#q=' + encodeURIComponent(q.value.trim()) : '';
      history.replaceState(null, '', location.pathname + location.search + h);
    }
  }

  function readHash() {
    const m = /^#q=(.*)$/.exec(location.hash);
    if (m) { try { q.value = decodeURIComponent(m[1]); } catch { q.value = m[1]; } }
  }

  q.addEventListener('input', () => apply(true));
  from.addEventListener('change', () => apply(false));
  to.addEventListener('change', () => apply(false));
  free.addEventListener('change', () => apply(false));
  famBtns.forEach((b) => b.addEventListener('click', () => {
    fam = b.dataset.fam;
    famBtns.forEach((o) => o.setAttribute('aria-pressed', o === b ? 'true' : 'false'));
    apply(false);
  }));
  addEventListener('hashchange', () => { if (/^#q=/.test(location.hash)) { readHash(); apply(false); } });
  readHash();
  apply(false);
}
