//! Copy Text (OCR): reading order and joining of recognised text boxes (pure).
//!
//! Boxes are normalised to 0..1 with the origin at the BOTTOM-left, as Vision returns them.

/// One recognised piece of text (usually a line).
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub confidence: f32,
}

impl TextBox {
    fn center_y(&self) -> f64 {
        self.y + self.h / 2.0
    }
    fn top(&self) -> f64 {
        self.y + self.h
    }
}

/// Orders boxes top-to-bottom in lines (a box joins a line when its vertical centre is within half
/// the smaller height of the line's first box), then left-to-right inside a line.
pub fn reading_order(boxes: &[TextBox]) -> Vec<Vec<&TextBox>> {
    let mut sorted: Vec<&TextBox> = boxes.iter().filter(|b| !b.text.trim().is_empty()).collect();
    sorted.sort_by(|a, b| b.top().total_cmp(&a.top()).then(a.x.total_cmp(&b.x)));
    let mut lines: Vec<Vec<&TextBox>> = vec![];
    for b in sorted {
        let joins = lines.last().and_then(|l| l.first()).is_some_and(|f| (b.center_y() - f.center_y()).abs() <= 0.5 * b.h.min(f.h));
        if joins {
            lines.last_mut().expect("checked above").push(b);
        } else {
            lines.push(vec![b]);
        }
    }
    for l in &mut lines {
        l.sort_by(|a, b| a.x.total_cmp(&b.x));
    }
    lines
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Joins boxes into text: words of a line with a space, lines with `\n`. A line ending in a hyphen
/// after a letter, followed by a line starting in lowercase, is rejoined without the hyphen.
pub fn join(boxes: &[TextBox]) -> String {
    let lines: Vec<String> = reading_order(boxes)
        .into_iter()
        .map(|l| collapse_spaces(&l.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" ")))
        .filter(|l| !l.is_empty())
        .collect();
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            let prev_hyphen = {
                let mut it = out.chars().rev();
                it.next() == Some('-') && it.next().is_some_and(char::is_alphabetic)
            };
            if prev_hyphen && line.chars().next().is_some_and(char::is_lowercase) {
                out.pop();
            } else {
                out.push('\n');
            }
        }
        out.push_str(line);
    }
    out
}

/// Toast after Copy Text: `Copied N characters` (characters, not bytes) or `No text found`.
pub fn toast(text: &str) -> String {
    match text.chars().count() {
        0 => "No text found".into(),
        n => format!("Copied {n} characters"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(text: &str, x: f64, y: f64, w: f64, h: f64) -> TextBox {
        TextBox { text: text.into(), x, y, w, h, confidence: 1.0 }
    }

    #[test]
    fn orders_top_to_bottom_then_left_to_right() {
        // bottom-left origin: larger y is higher on screen
        let boxes = [
            b("third", 0.1, 0.2, 0.2, 0.05),
            b("two", 0.5, 0.6005, 0.1, 0.05),
            b("one", 0.1, 0.6, 0.2, 0.05),
            b("second", 0.1, 0.4, 0.2, 0.05),
        ];
        assert_eq!(join(&boxes), "one two\nsecond\nthird");
    }

    #[test]
    fn separate_lines_are_not_merged_by_small_overlap() {
        let boxes = [b("a", 0.1, 0.50, 0.1, 0.05), b("b", 0.3, 0.44, 0.1, 0.05)];
        assert_eq!(join(&boxes), "a\nb");
    }

    #[test]
    fn rejoins_hyphenated_line_ends_and_collapses_spaces() {
        let boxes = [
            b("inter-", 0.1, 0.6, 0.2, 0.05),
            b("national  rules", 0.1, 0.5, 0.2, 0.05),
            b("A -", 0.1, 0.4, 0.2, 0.05),
            b("Next", 0.1, 0.3, 0.2, 0.05),
        ];
        assert_eq!(join(&boxes), "international rules\nA -\nNext");
    }

    #[test]
    fn empty_and_blank_boxes_give_no_text() {
        assert_eq!(join(&[]), "");
        assert_eq!(join(&[b("  ", 0.0, 0.0, 0.1, 0.1)]), "");
    }

    #[test]
    fn toast_counts_characters_not_bytes() {
        assert_eq!(toast(""), "No text found");
        assert_eq!(toast("abc"), "Copied 3 characters");
        assert_eq!(toast("café"), "Copied 4 characters");
        assert_eq!(toast("a😀é\nb"), "Copied 5 characters");
    }
}
