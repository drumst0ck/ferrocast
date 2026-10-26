// Notes section: a small real editable field. At the start of a line, "# ", "- ", "[] " and "**bold**" turn into the
// formatting the Notes window documents (heading, bullet, task, bold). Without script the field is a static sample.
(() => {
  const root = document.querySelector('[data-notes-field]');
  if (!root) return;
  const ZW = '\u200b';
  const sel = () => getSelection();

  root.contentEditable = 'true';
  root.spellcheck = false;
  root.setAttribute('role', 'textbox');
  root.setAttribute('aria-multiline', 'true');
  root.setAttribute('aria-label', root.dataset.label || '');
  if (root.dataset.describedby) root.setAttribute('aria-describedby', root.dataset.describedby);
  root.dataset.live = '';
  Array.from(root.childNodes).forEach((n) => { if (n.nodeType === 3 && !n.textContent.trim()) root.removeChild(n); });

  const textOf = (el) => el.textContent.split(ZW).join('').replace(/\u00a0/g, ' ');
  function blockOf(node) {
    while (node && node.parentNode !== root) node = node.parentNode;
    return node && node.nodeType === 1 ? node : null;
  }
  function currentBlock() {
    const s = sel();
    if (!s || !s.rangeCount || !root.contains(s.anchorNode)) return null;
    if (s.anchorNode === root) return root.children[Math.min(s.anchorOffset, root.children.length - 1)] || null;
    return blockOf(s.anchorNode);
  }
  function makeBlock(kind) {
    const b = document.createElement('div');
    b.dataset.k = kind;
    b.appendChild(document.createElement('br'));
    return b;
  }
  function caretAtStart(el) {
    const r = document.createRange();
    r.selectNodeContents(el);
    r.collapse(true);
    const s = sel();
    s.removeAllRanges();
    s.addRange(r);
  }
  function setKind(block, kind, level) {
    block.dataset.k = kind;
    if (level) block.dataset.l = String(level); else delete block.dataset.l;
    if (kind !== 't') delete block.dataset.done;
  }
  function clearBlock(block) {
    block.textContent = '';
    block.appendChild(document.createElement('br'));
    caretAtStart(block);
  }
  function normalise() {
    if (!root.children.length) { root.appendChild(makeBlock('p')); caretAtStart(root.firstChild); }
    Array.from(root.childNodes).forEach((n) => {
      if (n.nodeType === 3 && !n.textContent.trim()) { root.removeChild(n); return; } // markup whitespace between blocks
      if (n.nodeType === 3 || (n.nodeType === 1 && !n.dataset.k)) {
        const b = makeBlock('p');
        root.replaceChild(b, n);
        b.textContent = n.textContent;
        if (!b.textContent) b.appendChild(document.createElement('br'));
      }
    });
  }

  root.addEventListener('input', () => {
    normalise();
    const s = sel();
    const block = currentBlock();
    if (!block || !s.isCollapsed) return;
    const text = textOf(block);
    let m;
    if (block.dataset.k === 'p' || block.dataset.k === 'b') {
      if (block.dataset.k === 'p' && (m = text.match(/^(#{1,3}) $/))) { setKind(block, 'h', m[1].length); clearBlock(block); return; }
      if (block.dataset.k === 'p' && /^[-*] $/.test(text)) { setKind(block, 'b'); clearBlock(block); return; }
      if ((m = text.match(/^\[([ xX]?)\] $/))) { setKind(block, 't'); if (m[1].trim()) block.dataset.done = 'true'; clearBlock(block); return; }
    }
    const n = s.anchorNode;
    if (n && n.nodeType === 3 && s.anchorOffset === n.length && !(n.parentNode.closest && n.parentNode.closest('strong'))) {
      m = n.data.match(/(\*\*|__)([^*_\s](?:[^*_]*[^*_\s])?)\1$/);
      if (m) {
        const r = document.createRange();
        r.setStart(n, n.length - m[0].length);
        r.setEnd(n, n.length);
        r.deleteContents();
        const z = document.createTextNode(ZW);
        const b = document.createElement('strong');
        b.textContent = m[2];
        r.insertNode(z);
        r.insertNode(b);
        const c = document.createRange();
        c.setStart(z, 1);
        c.collapse(true);
        s.removeAllRanges();
        s.addRange(c);
      }
    }
  });

  function atBlockStart(block) {
    const s = sel();
    if (!s.isCollapsed) return false;
    const r = document.createRange();
    r.selectNodeContents(block);
    r.setEnd(s.anchorNode, s.anchorOffset);
    return r.toString().split(ZW).join('') === '';
  }

  root.addEventListener('beforeinput', (e) => {
    const block = currentBlock();
    if (!block) return;
    const kind = block.dataset.k;
    if (e.inputType === 'insertParagraph' || e.inputType === 'insertLineBreak') {
      e.preventDefault();
      if (kind !== 'p' && textOf(block) === '') { setKind(block, 'p'); return; } // Enter on an empty bullet or task leaves the list
      const next = makeBlock(kind === 'b' || kind === 't' ? kind : 'p');
      const s = sel();
      const r = document.createRange();
      r.setStart(s.anchorNode, s.anchorOffset);
      r.setEndAfter(block.lastChild);
      const rest = r.extractContents();
      if (textOf({ textContent: rest.textContent }) !== '') { next.textContent = ''; next.appendChild(rest); }
      if (!block.lastChild) block.appendChild(document.createElement('br'));
      block.after(next);
      caretAtStart(next);
    } else if (e.inputType === 'deleteContentBackward' && kind !== 'p' && atBlockStart(block)) {
      e.preventDefault();
      setKind(block, 'p'); // Backspace at the start of a formatted line gives the plain line back
    }
  });

  function toggleTask(block) {
    if (block.dataset.done) delete block.dataset.done; else block.dataset.done = 'true';
  }
  root.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      const block = currentBlock();
      if (block && block.dataset.k === 't') { e.preventDefault(); toggleTask(block); }
    }
  });
  root.addEventListener('click', (e) => {
    const t = e.target;
    if (t && t.dataset && t.dataset.k === 't' && e.offsetX <= 30) toggleTask(t);
  });
  root.addEventListener('paste', (e) => {
    e.preventDefault();
    const text = (e.clipboardData || window.clipboardData).getData('text').replace(/\s*\n\s*/g, ' ');
    document.execCommand('insertText', false, text);
  });
})();
