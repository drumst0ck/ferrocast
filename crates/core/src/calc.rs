//! Root Search calculator: arithmetic, functions, percentages, natural
//! language helpers and unit conversion.

use crate::units;

#[derive(Debug, Clone, PartialEq)]
pub struct CalcResult {
    /// The (normalised) question, e.g. `10 ft in m`.
    pub question: String,
    pub value: f64,
    /// Unit symbol of the answer, if any.
    pub unit: Option<String>,
    /// Human formatted answer (`3.048 m`).
    pub display: String,
    /// Answer without grouping separators or unit (`3.048`).
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') {
                i += 1;
            }
            // scientific notation
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    i = j;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let text: String = chars[start..i].iter().filter(|c| **c != '_').collect();
            let mut v: f64 = text.parse().ok()?;
            // Shorthand suffixes: 10k, 2.5M, 1B (only if not followed by a letter)
            if i < chars.len() && !chars.get(i + 1).is_some_and(|c| c.is_alphabetic()) {
                let mult = match chars[i] {
                    'k' | 'K' => Some(1e3),
                    'M' => Some(1e6),
                    _ => None,
                };
                if let Some(m) = mult {
                    v *= m;
                    i += 1;
                }
            }
            out.push(Tok::Num(v));
        } else if c.is_alphabetic() || c == '°' || c == '²' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '°' || chars[i] == '²' || chars[i] == '/') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect::<String>().to_lowercase()));
        } else {
            match c {
                '+' | '-' | '*' | '/' | '^' | '%' | '!' => out.push(Tok::Op(c)),
                '×' | 'x' => out.push(Tok::Op('*')),
                '÷' => out.push(Tok::Op('/')),
                '−' => out.push(Tok::Op('-')),
                '(' => out.push(Tok::LParen),
                ')' => out.push(Tok::RParen),
                ',' => out.push(Tok::Comma),
                _ => return None,
            }
            i += 1;
        }
    }
    Some(out)
}

/// A value during evaluation; `percent` marks a trailing `%` for `a + b%`.
#[derive(Debug, Clone, Copy)]
struct Val {
    v: f64,
    percent: bool,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat_op(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_ident(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(s)) if s == w) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<Val> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op('+')) => '+',
                Some(Tok::Op('-')) => '-',
                Some(Tok::Ident(w)) if w == "plus" => '+',
                Some(Tok::Ident(w)) if w == "minus" => '-',
                _ => break,
            };
            self.pos += 1;
            let rhs = self.term()?;
            let a = lhs.v;
            let b = if rhs.percent && !lhs.percent { a * rhs.v / 100.0 } else { rhs.v };
            lhs = Val { v: if op == '+' { a + b } else { a - b }, percent: lhs.percent && rhs.percent };
        }
        Some(lhs)
    }

    fn term(&mut self) -> Option<Val> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op('*')) => '*',
                Some(Tok::Op('/')) => '/',
                Some(Tok::Ident(w)) if w == "times" => '*',
                Some(Tok::Ident(w)) if w == "mod" => 'm',
                // `20% of 80`, `20% off 80`
                Some(Tok::Ident(w)) if w == "of" && lhs.percent => 'o',
                Some(Tok::Ident(w)) if w == "off" && lhs.percent => 'f',
                // implicit multiplication: `2(3+4)`, `2pi`
                Some(Tok::LParen) => 'i',
                Some(Tok::Ident(w)) if is_const(w) || is_func(w) => 'i',
                _ => break,
            };
            if op != 'i' {
                self.pos += 1;
            }
            let rhs = self.unary()?;
            let a = if lhs.percent && (op == '*' || op == '/') { lhs.v / 100.0 } else { lhs.v };
            lhs = match op {
                '*' | 'i' => Val { v: a * rhs.v, percent: false },
                '/' => Val { v: a / rhs.v, percent: false },
                'm' => Val { v: a.rem_euclid(rhs.v), percent: false },
                'o' => Val { v: lhs.v / 100.0 * rhs.v, percent: false },
                'f' => Val { v: rhs.v * (1.0 - lhs.v / 100.0), percent: false },
                _ => unreachable!(),
            };
        }
        Some(lhs)
    }

    fn unary(&mut self) -> Option<Val> {
        if self.eat_op('-') {
            let v = self.unary()?;
            return Some(Val { v: -v.v, percent: v.percent });
        }
        if self.eat_op('+') {
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Option<Val> {
        let base = self.postfix()?;
        let is_pow = self.eat_op('^') || self.eat_ident("power") || {
            // `2 to the power of 10`
            if self.eat_ident("to") {
                if self.eat_ident("the") && self.eat_ident("power") {
                    self.eat_ident("of");
                    true
                } else {
                    self.pos -= 1;
                    false
                }
            } else {
                false
            }
        };
        if is_pow {
            self.eat_ident("of");
            let exp = self.unary()?;
            return Some(Val { v: base.v.powf(exp.v), percent: false });
        }
        Some(base)
    }

    fn postfix(&mut self) -> Option<Val> {
        let mut v = self.primary()?;
        loop {
            if self.eat_op('%') {
                v.percent = true;
            } else if self.eat_op('!') {
                v.v = factorial(v.v)?;
            } else if self.eat_ident("squared") {
                v.v *= v.v;
            } else if self.eat_ident("cubed") {
                v.v = v.v.powi(3);
            } else {
                break;
            }
        }
        Some(v)
    }

    fn primary(&mut self) -> Option<Val> {
        match self.next()? {
            Tok::Num(n) => Some(Val { v: n, percent: false }),
            Tok::LParen => {
                let v = self.expr()?;
                if self.next()? != Tok::RParen {
                    return None;
                }
                Some(v)
            }
            Tok::Ident(w) => {
                if let Some(c) = constant(&w) {
                    return Some(Val { v: c, percent: false });
                }
                // `square root of 625`, `cube root of 27`
                if (w == "square" || w == "cube") && self.eat_ident("root") {
                    self.eat_ident("of");
                    let x = self.unary()?.v;
                    return Some(Val { v: if w == "square" { x.sqrt() } else { x.cbrt() }, percent: false });
                }
                if is_func(&w) {
                    self.eat_ident("of");
                    let args = if self.peek() == Some(&Tok::LParen) {
                        self.pos += 1;
                        let mut args = vec![self.expr()?.v];
                        while self.peek() == Some(&Tok::Comma) {
                            self.pos += 1;
                            args.push(self.expr()?.v);
                        }
                        if self.next()? != Tok::RParen {
                            return None;
                        }
                        args
                    } else {
                        vec![self.power()?.v]
                    };
                    return Some(Val { v: apply(&w, &args)?, percent: false });
                }
                None
            }
            _ => None,
        }
    }
}

fn factorial(n: f64) -> Option<f64> {
    if n < 0.0 || n.fract() != 0.0 || n > 170.0 {
        return None;
    }
    Some((1..=n as u64).fold(1.0, |acc, k| acc * k as f64))
}

fn constant(w: &str) -> Option<f64> {
    match w {
        "pi" | "π" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" | "τ" => Some(std::f64::consts::TAU),
        "phi" => Some(1.618_033_988_749_895),
        _ => None,
    }
}
fn is_const(w: &str) -> bool {
    constant(w).is_some()
}

const FUNCS: &[&str] = &[
    "sqrt", "cbrt", "abs", "floor", "ceil", "round", "trunc", "exp", "ln", "log", "log2", "log10", "sin", "cos", "tan", "cot", "sec",
    "csc", "asin", "acos", "atan", "acot", "sinh", "cosh", "tanh", "coth", "asinh", "acosh", "atanh", "min", "max", "pow", "fact", "root",
];
fn is_func(w: &str) -> bool {
    FUNCS.contains(&w)
}

fn apply(f: &str, a: &[f64]) -> Option<f64> {
    let x = *a.first()?;
    let one = a.len() == 1;
    Some(match f {
        "sqrt" if one => x.sqrt(),
        "cbrt" if one => x.cbrt(),
        "abs" if one => x.abs(),
        "floor" if one => x.floor(),
        "ceil" if one => x.ceil(),
        "round" if one => x.round(),
        "round" => {
            let p = 10f64.powi(a[1] as i32);
            (x * p).round() / p
        }
        "trunc" if one => x.trunc(),
        "exp" if one => x.exp(),
        "ln" if one => x.ln(),
        "log" | "log10" if one => x.log10(),
        "log" => x.ln() / a[1].ln(),
        "log2" if one => x.log2(),
        "sin" if one => x.sin(),
        "cos" if one => x.cos(),
        "tan" if one => x.tan(),
        "cot" if one => 1.0 / x.tan(),
        "sec" if one => 1.0 / x.cos(),
        "csc" if one => 1.0 / x.sin(),
        "asin" if one => x.asin(),
        "acos" if one => x.acos(),
        "atan" if one => x.atan(),
        "acot" if one => (1.0 / x).atan(),
        "sinh" if one => x.sinh(),
        "cosh" if one => x.cosh(),
        "tanh" if one => x.tanh(),
        "coth" if one => 1.0 / x.tanh(),
        "asinh" if one => x.asinh(),
        "acosh" if one => x.acosh(),
        "atanh" if one => x.atanh(),
        "min" => a.iter().copied().fold(f64::INFINITY, f64::min),
        "max" => a.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "pow" if a.len() == 2 => x.powf(a[1]),
        "root" if a.len() == 2 => x.powf(1.0 / a[1]),
        "fact" if one => factorial(x)?,
        _ => return None,
    })
}

fn eval_math(s: &str) -> Option<f64> {
    let toks = tokenize(s)?;
    if toks.is_empty() {
        return None;
    }
    let mut p = Parser { toks, pos: 0 };
    let v = p.expr()?;
    if p.pos != p.toks.len() {
        return None;
    }
    let r = if v.percent { v.v / 100.0 } else { v.v };
    r.is_finite().then_some(r)
}

/// Format a number with up to 10 significant decimals and thousands grouping.
pub fn format_number(v: f64, group: bool) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let abs = v.abs();
    if !(1e-9..1e15).contains(&abs) {
        let s = format!("{v:e}");
        return s;
    }
    let decimals = if abs >= 1.0 { (10 - abs.log10().floor() as i32).clamp(0, 10) } else { 10 };
    let mut s = format!("{:.*}", decimals as usize, v);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    if !group {
        return s;
    }
    let (sign, rest) = if let Some(r) = s.strip_prefix('-') { ("-", r) } else { ("", s.as_str()) };
    let (int, frac) = rest.split_once('.').map_or((rest, None), |(a, b)| (a, Some(b)));
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Does the input look like something the user wants calculated?
fn looks_like_math(s: &str) -> bool {
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_op = s.chars().any(|c| "+-*/^%!×÷()".contains(c));
    let lower = s.to_lowercase();
    let words = [
        "sqrt", "root", "sin", "cos", "tan", "log", "ln", "pi", " of ", " off ", "power", "squared", "cubed", "abs", "mod", "min", "max",
        "plus", "minus", "times", "fact", "floor", "ceil", "round", "exp",
    ];
    (has_digit || lower.contains("pi")) && (has_op || words.iter().any(|w| lower.contains(w)))
}

/// Try to split `"<expr> <unit> in|to|as <unit>"`.
fn try_conversion(s: &str, rem_px: f64) -> Option<CalcResult> {
    let lower = s.to_lowercase();
    let (lhs, rhs) = [" in ", " to ", " as ", " → ", " -> "]
        .iter()
        .find_map(|sep| lower.rfind(sep).map(|i| (lower[..i].trim().to_string(), lower[i + sep.len()..].trim().to_string())))?;
    let to = units::find(&rhs)?;
    // Find the longest unit alias at the end of lhs.
    let (num_part, from) = units::aliases_longest_first().into_iter().find_map(|(alias, unit)| {
        let stripped = lhs.strip_suffix(alias)?;
        // Must be separated from a digit/space/paren, not part of a word
        let ok = stripped.chars().last().is_some_and(|c| c.is_ascii_digit() || c == ' ' || c == ')' || c == '.');
        ok.then(|| (stripped.trim().to_string(), unit))
    })?;
    let value = if num_part.is_empty() { 1.0 } else { eval_math(&num_part)? };
    let out = units::convert(value, from, to, rem_px)?;
    let display_unit = to.symbol.to_string();
    Some(CalcResult {
        question: format!("{} {} → {}", format_number(value, true), from.symbol, to.symbol),
        value: out,
        unit: Some(display_unit.clone()),
        display: format!("{} {}", format_number(out, true), display_unit),
        raw: format_number(out, false),
    })
}

/// `100 usd in eur`, `10k gbp to jpy`.
fn try_currency(s: &str, rates: &crate::currency::Rates) -> Option<CalcResult> {
    let lower = s.to_lowercase();
    let (lhs, rhs) = [" in ", " to ", " as ", " → ", " -> "]
        .iter()
        .find_map(|sep| lower.rfind(sep).map(|i| (lower[..i].trim().to_string(), lower[i + sep.len()..].trim().to_string())))?;
    let to = rhs.to_uppercase();
    if to.len() != 3 || !to.chars().all(|c| c.is_ascii_alphabetic()) || !rates.knows(&to) {
        return None;
    }
    let split = lhs.len().checked_sub(3)?;
    if !lhs.is_char_boundary(split) {
        return None;
    }
    let (num, from) = (lhs[..split].trim(), lhs[split..].to_uppercase());
    if !rates.knows(&from) || !num.chars().last().is_none_or(|c| c.is_ascii_digit() || c == ')' || c == '.' || c.is_ascii_alphabetic()) {
        return None;
    }
    let value = if num.is_empty() { 1.0 } else { eval_math(num)? };
    let out = rates.convert(value, &from, &to)?;
    Some(CalcResult {
        question: format!("{} {from} → {to}", format_number(value, true)),
        value: out,
        unit: Some(to.clone()),
        display: format!("{} {to}", format_number((out * 100.0).round() / 100.0, true)),
        raw: format_number((out * 100.0).round() / 100.0, false),
    })
}

/// Evaluate a Root Search query. Returns `None` if it isn't a calculation.
pub fn evaluate(input: &str, rem_px: f64) -> Option<CalcResult> {
    evaluate_with_rates(input, rem_px, None)
}

/// Like [`evaluate`], additionally understanding currency conversions when rates are available.
pub fn evaluate_with_rates(input: &str, rem_px: f64, rates: Option<&crate::currency::Rates>) -> Option<CalcResult> {
    let s = input.trim().trim_end_matches('=').trim();
    if s.is_empty() || s.len() > 200 {
        return None;
    }
    if let Some(r) = rates.and_then(|r| try_currency(s, r)) {
        return Some(r);
    }
    if let Some(r) = try_conversion(s, rem_px) {
        return Some(r);
    }
    if !looks_like_math(s) {
        return None;
    }
    let v = eval_math(s)?;
    Some(CalcResult { question: s.to_string(), value: v, unit: None, display: format_number(v, true), raw: format_number(v, false) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        evaluate(s, 16.0).unwrap_or_else(|| panic!("no result for {s}")).value
    }
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn arithmetic() {
        assert!(close(ev("128 * 3.5"), 448.0));
        assert!(close(ev("2 + 3 * 4"), 14.0));
        assert!(close(ev("(2 + 3) * 4"), 20.0));
        assert!(close(ev("2^10"), 1024.0));
        assert!(close(ev("2 power 10"), 1024.0));
        assert!(close(ev("-3 + 5"), 2.0));
        assert!(close(ev("10 / 4"), 2.5));
        assert!(close(ev("5!"), 120.0));
        assert!(close(ev("2(3+4)"), 14.0));
    }

    #[test]
    fn percentages() {
        assert!(close(ev("52% of 900"), 468.0));
        assert!(close(ev("20% off 80"), 64.0));
        assert!(close(ev("100 + 10%"), 110.0));
        assert!(close(ev("100 - 10%"), 90.0));
    }

    #[test]
    fn functions() {
        assert!(close(ev("square root of 625"), 25.0));
        assert!(close(ev("sqrt(16) + 1"), 5.0));
        assert!(close(ev("sin(0)"), 0.0));
        assert!(close(ev("max(1, 7, 3)"), 7.0));
        assert!(close(ev("2pi"), std::f64::consts::TAU));
    }

    #[test]
    fn conversions() {
        let r = evaluate("10ft in m", 16.0).unwrap();
        assert!(close(r.value, 3.048));
        assert_eq!(r.unit.as_deref(), Some("m"));
        assert!(close(ev("100 c to f"), 212.0));
        assert!(close(ev("2rem in px"), 32.0));
        assert!(close(ev("1 gb in mb"), 1000.0));
        assert!(close(ev("5 km to miles"), 3.106855961811784));
    }

    #[test]
    fn currency() {
        let rates = crate::currency::Rates::parse(r#"{"base":"EUR","date":"d","rates":{"USD":1.25,"GBP":0.8}}"#, 0).unwrap();
        let r = evaluate_with_rates("100 usd in eur", 16.0, Some(&rates)).unwrap();
        assert!((r.value - 80.0).abs() < 1e-9);
        assert_eq!(r.display, "80 EUR");
        let r = evaluate_with_rates("2k gbp to usd", 16.0, Some(&rates)).unwrap();
        assert_eq!(r.raw, "3125");
        assert!(evaluate_with_rates("100 usd in xyz", 16.0, Some(&rates)).is_none());
        assert!(evaluate_with_rates("100 usd in eur", 16.0, None).is_none());
        // unit conversions keep working alongside currencies
        assert!(evaluate_with_rates("10ft in m", 16.0, Some(&rates)).is_some());
    }

    #[test]
    fn not_math() {
        assert!(evaluate("safari", 16.0).is_none());
        assert!(evaluate("42", 16.0).is_none());
        assert!(evaluate("open in finder", 16.0).is_none());
        assert!(evaluate("1password", 16.0).is_none());
    }

    #[test]
    fn formatting() {
        assert_eq!(format_number(1234567.5, true), "1,234,567.5");
        assert_eq!(format_number(0.1 + 0.2, true), "0.3");
        assert_eq!(format_number(-1000.0, true), "-1,000");
        assert_eq!(format_number(2.0 / 3.0, false), "0.6666666667");
    }
}
