/*
 * Home launcher demo helpers. Plain ES5-style script: vitest imports it, and HomePage.astro inlines its
 * source (without the `export` keywords) so the page runs no dynamic code (CSP-safe).
 */

/** Evaluate + - * / % ^ and parentheses. Returns a finite number or null. */
export function evalExpr(input) {
  var s = String(input).trim().replace(/,/g, '.');
  var i = 0;
  function peek() { while (s.charAt(i) === ' ') i++; return s.charAt(i); }
  function num() {
    peek();
    var m = /^\d+(?:\.\d+)?|^\.\d+/.exec(s.slice(i));
    if (!m) throw new Error('num');
    i += m[0].length;
    return parseFloat(m[0]);
  }
  function atom() {
    if (peek() === '(') { i++; var v = add(); if (peek() !== ')') throw new Error(')'); i++; return v; }
    return num();
  }
  function unary() {
    if (peek() === '-') { i++; return -unary(); }
    if (peek() === '+') { i++; return unary(); }
    var b = atom();
    if (peek() === '^') { i++; return Math.pow(b, unary()); }
    return b;
  }
  function mul() {
    var v = unary();
    while (peek() === '*' || peek() === '/' || peek() === '%') {
      var op = s.charAt(i++), r = unary();
      v = op === '*' ? v * r : op === '/' ? v / r : v % r;
    }
    return v;
  }
  function add() {
    var v = mul();
    while (peek() === '+' || peek() === '-') { var op = s.charAt(i++), r = mul(); v = op === '+' ? v + r : v - r; }
    return v;
  }
  try {
    var out = add();
    peek();
    if (i !== s.length || !isFinite(out)) return null;
    return out;
  } catch (e) { return null; }
}

/** Two-decimal number in the page language. */
export function fmtNum(n, lang) {
  return (Math.round(n * 100) / 100).toLocaleString(lang === 'es' ? 'es-ES' : 'en-US', { maximumFractionDigits: 2 });
}
