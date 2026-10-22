// Moves the red glow of the footer wordmark with the pointer (fine pointers only).
const mark = document.querySelector('.foot-mark');
if (mark && matchMedia('(hover:hover) and (pointer:fine)').matches) {
  mark.addEventListener('pointermove', (e) => {
    const r = mark.getBoundingClientRect();
    mark.style.setProperty('--mx', e.clientX - r.left + 'px');
    mark.style.setProperty('--my', e.clientY - r.top + 'px');
  });
  mark.addEventListener('pointerleave', () => {
    mark.style.removeProperty('--mx');
    mark.style.removeProperty('--my');
  });
}
