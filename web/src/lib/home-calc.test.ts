import { describe, expect, it } from 'vitest';
// @ts-expect-error plain browser script shared with the home demo
import { evalExpr, fmtNum } from './home-calc.js';

describe('home demo expression evaluator (no eval / Function)', () => {
  it('evaluates arithmetic with precedence, parentheses and powers', () => {
    expect(evalExpr('2+3*4')).toBe(14);
    expect(evalExpr('(2+3)*4')).toBe(20);
    expect(evalExpr('2^3^2')).toBe(512);
    expect(evalExpr('10/4')).toBe(2.5);
    expect(evalExpr('7%4')).toBe(3);
    expect(evalExpr('-3+5')).toBe(2);
    expect(evalExpr('1,5*2')).toBe(3);
  });
  it('returns null for malformed or non-finite input', () => {
    expect(evalExpr('2+')).toBeNull();
    expect(evalExpr('(2+3')).toBeNull();
    expect(evalExpr('1/0')).toBeNull();
    expect(evalExpr('2 3')).toBeNull();
    expect(evalExpr('alert(1)')).toBeNull();
  });
  it('does not use Function or eval', async () => {
    const { readFileSync } = await import('node:fs');
    const src = readFileSync(new URL('./home-calc.js', import.meta.url), 'utf8');
    expect(src).not.toMatch(/Function\s*\(|\beval\s*\(/);
  });
});

describe('fmtNum', () => {
  it('uses the page language for separators', () => {
    expect(fmtNum(1234567.891, 'en')).toBe('1,234,567.89');
    expect(fmtNum(1234567.891, 'es')).toBe('1.234.567,89');
  });
});
