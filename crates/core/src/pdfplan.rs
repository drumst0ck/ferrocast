//! State of the visual PDF organizer (Tangerine organizePDF): an ordered list of source pages,
//! each with a rotation. Pure, so every edit is testable without a window or a PDF.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// 0-based page of the source document.
    pub src: usize,
    /// Clockwise quarter turns, 0..=3.
    pub turns: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfPlan {
    pub pages: Vec<Page>,
    /// Page the keyboard acts on.
    pub cursor: usize,
}

impl PdfPlan {
    /// Every page of an `n`-page document, in order, unrotated.
    pub fn new(n: usize) -> Self {
        PdfPlan { pages: (0..n).map(|src| Page { src, turns: 0 }).collect(), cursor: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// True when the result would equal the original (nothing to write).
    pub fn is_identity(&self, n: usize) -> bool {
        self.pages.len() == n && self.pages.iter().enumerate().all(|(i, p)| p.src == i && p.turns == 0)
    }

    pub fn move_cursor(&mut self, by: isize) {
        if self.pages.is_empty() {
            return;
        }
        self.cursor = (self.cursor as isize + by).clamp(0, self.pages.len() as isize - 1) as usize;
    }

    /// Rotate the current page by `quarters` (negative is counter-clockwise).
    pub fn rotate(&mut self, quarters: i32) {
        if let Some(p) = self.pages.get_mut(self.cursor) {
            p.turns = (p.turns + quarters).rem_euclid(4);
        }
    }

    /// Insert a copy right after the current page and select it.
    pub fn duplicate(&mut self) {
        if let Some(p) = self.pages.get(self.cursor).copied() {
            self.pages.insert(self.cursor + 1, p);
            self.cursor += 1;
        }
    }

    /// Drop the current page. The last page cannot be removed: an empty PDF is not a result.
    pub fn remove(&mut self) -> bool {
        if self.pages.len() <= 1 {
            return false;
        }
        self.pages.remove(self.cursor);
        self.cursor = self.cursor.min(self.pages.len() - 1);
        true
    }

    /// Move the current page one place earlier (`-1`) or later (`1`); the cursor follows it.
    pub fn shift(&mut self, by: isize) {
        let to = self.cursor as isize + by;
        if self.pages.is_empty() || to < 0 || to >= self.pages.len() as isize {
            return;
        }
        self.pages.swap(self.cursor, to as usize);
        self.cursor = to as usize;
    }

    /// Selection for the writer: (source index, quarter turns).
    pub fn selection(&self) -> Vec<(usize, i32)> {
        self.pages.iter().map(|p| (p.src, p.turns)).collect()
    }

    /// Label for the page strip, e.g. `3` or `3 ↻90`.
    pub fn label(p: &Page) -> String {
        if p.turns == 0 {
            format!("{}", p.src + 1)
        } else {
            format!("{} ↻{}", p.src + 1, p.turns * 90)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srcs(p: &PdfPlan) -> Vec<usize> {
        p.pages.iter().map(|x| x.src).collect()
    }

    #[test]
    fn starts_as_the_original() {
        let p = PdfPlan::new(3);
        assert_eq!(srcs(&p), [0, 1, 2]);
        assert!(p.is_identity(3));
        assert!(!p.is_identity(4));
        assert!(PdfPlan::new(0).is_empty());
    }

    #[test]
    fn rotation_wraps_both_ways() {
        let mut p = PdfPlan::new(2);
        p.rotate(1);
        p.rotate(1);
        assert_eq!(p.pages[0].turns, 2);
        p.rotate(-3);
        assert_eq!(p.pages[0].turns, 3, "2 - 3 wraps to 3");
        p.rotate(1);
        assert_eq!(p.pages[0].turns, 0);
        assert!(p.is_identity(2));
        assert_eq!(PdfPlan::label(&Page { src: 2, turns: 3 }), "3 ↻270");
        assert_eq!(PdfPlan::label(&Page { src: 0, turns: 0 }), "1");
    }

    #[test]
    fn duplicate_copies_rotation_and_selects_the_copy() {
        let mut p = PdfPlan::new(3);
        p.move_cursor(1);
        p.rotate(1);
        p.duplicate();
        assert_eq!(srcs(&p), [0, 1, 1, 2]);
        assert_eq!(p.pages[2].turns, 1);
        assert_eq!(p.cursor, 2);
        assert_eq!(p.selection(), [(0, 0), (1, 1), (1, 1), (2, 0)]);
    }

    #[test]
    fn remove_keeps_at_least_one_page() {
        let mut p = PdfPlan::new(3);
        p.move_cursor(2);
        assert!(p.remove());
        assert_eq!((srcs(&p), p.cursor), (vec![0, 1], 1), "cursor stays on a page");
        assert!(p.remove());
        assert!(!p.remove(), "the last page stays");
        assert_eq!(srcs(&p), [0]);
    }

    #[test]
    fn shifting_reorders_and_stops_at_the_ends() {
        let mut p = PdfPlan::new(3);
        p.shift(-1);
        assert_eq!((srcs(&p), p.cursor), (vec![0, 1, 2], 0), "nothing before the first");
        p.shift(1);
        p.shift(1);
        assert_eq!((srcs(&p), p.cursor), (vec![1, 2, 0], 2));
        p.shift(1);
        assert_eq!(srcs(&p), [1, 2, 0]);
        assert!(!p.is_identity(3));
    }

    #[test]
    fn cursor_is_clamped_and_empty_is_safe() {
        let mut p = PdfPlan::new(2);
        p.move_cursor(-5);
        assert_eq!(p.cursor, 0);
        p.move_cursor(9);
        assert_eq!(p.cursor, 1);
        let mut e = PdfPlan::new(0);
        e.move_cursor(1);
        e.rotate(1);
        e.duplicate();
        e.shift(1);
        assert!(!e.remove());
        assert!(e.is_empty());
    }
}
