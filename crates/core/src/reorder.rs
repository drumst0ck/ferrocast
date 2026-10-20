//! Reordering a list of files (merge PDFs, join videos, build a PDF from pictures). Pure index
//! maths, so the rules are testable without a window; the drag gesture only supplies indices.

/// Move the element at `from` so it ends up at index `to`. `false` (and no change) when either
/// index is out of range.
pub fn move_item<T>(v: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from >= v.len() || to >= v.len() {
        return false;
    }
    let item = v.remove(from);
    v.insert(to, item);
    true
}

/// Move one step towards the start; the new index, or `None` when already first or out of range.
pub fn move_up<T>(v: &mut Vec<T>, i: usize) -> Option<usize> {
    (i > 0 && i < v.len() && move_item(v, i, i - 1)).then(|| i - 1)
}

/// Move one step towards the end; the new index, or `None` when already last or out of range.
pub fn move_down<T>(v: &mut Vec<T>, i: usize) -> Option<usize> {
    (i + 1 < v.len() && move_item(v, i, i + 1)).then(|| i + 1)
}

/// Final index of the dragged row `from` when it is dropped in `slot`, where slot `k` is the gap
/// before row `k` (0 = before the first row, `len` = after the last). Both arguments are clamped,
/// so a pointer past either end never panics.
pub fn drop_index(from: usize, slot: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let from = from.min(len - 1);
    let slot = slot.min(len);
    // removing the row shifts every later slot down by one
    let to = if slot > from { slot - 1 } else { slot };
    to.min(len - 1)
}

/// Files in the order they were selected (Finder gives the pick order), without repeats.
pub fn finder_order(paths: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::with_capacity(paths.len());
    for p in paths {
        if !out.contains(p) {
            out.push(p.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn v() -> Vec<char> {
        vec!['a', 'b', 'c', 'd']
    }

    #[test]
    fn move_item_puts_the_element_at_the_target_index() {
        let mut x = v();
        assert!(move_item(&mut x, 0, 2));
        assert_eq!(x, ['b', 'c', 'a', 'd']);
        assert!(move_item(&mut x, 3, 0));
        assert_eq!(x, ['d', 'b', 'c', 'a']);
        assert!(move_item(&mut x, 1, 1), "moving onto itself is allowed and changes nothing");
        assert_eq!(x, ['d', 'b', 'c', 'a']);
    }

    #[test]
    fn out_of_range_moves_do_nothing_and_do_not_panic() {
        let mut x = v();
        assert!(!move_item(&mut x, 4, 0));
        assert!(!move_item(&mut x, 0, 4));
        assert!(!move_item(&mut x, 9, 9));
        assert_eq!(x, v());
        let mut empty: Vec<char> = vec![];
        assert!(!move_item(&mut empty, 0, 0));
        let mut one = vec!['a'];
        assert!(move_item(&mut one, 0, 0));
        assert_eq!(one, ['a']);
    }

    #[test]
    fn up_and_down_step_by_one_and_stop_at_the_ends() {
        let mut x = v();
        assert_eq!(move_down(&mut x, 0), Some(1));
        assert_eq!(x, ['b', 'a', 'c', 'd']);
        assert_eq!(move_up(&mut x, 1), Some(0));
        assert_eq!(x, v());
        assert_eq!(move_up(&mut x, 0), None, "already first");
        assert_eq!(move_down(&mut x, 3), None, "already last");
        assert_eq!(move_down(&mut x, 7), None, "out of range");
        assert_eq!(x, v());
    }

    #[test]
    fn dropping_in_a_slot_gives_the_final_index() {
        // slots are the gaps between rows: 0 is before the first row, len after the last
        assert_eq!(drop_index(0, 0, 4), 0, "dropped where it was");
        assert_eq!(drop_index(0, 1, 4), 0, "the slot right after itself is also a no-op");
        assert_eq!(drop_index(0, 3, 4), 2, "removal shifts later slots down by one");
        assert_eq!(drop_index(0, 4, 4), 3, "after the last row");
        assert_eq!(drop_index(3, 0, 4), 0);
        assert_eq!(drop_index(3, 1, 4), 1);
        assert_eq!(drop_index(3, 4, 4), 3, "after itself at the end is a no-op");
        assert_eq!(drop_index(2, 99, 4), 3, "a slot past the end clamps to the last index");
        assert_eq!(drop_index(5, 0, 4), 0, "an out-of-range source clamps instead of panicking");
    }

    #[test]
    fn dropping_through_move_item_matches_the_slot_maths() {
        for from in 0..4 {
            for slot in 0..=4 {
                let mut x = v();
                let to = drop_index(from, slot, 4);
                assert!(move_item(&mut x, from, to), "{from} {slot}");
                let mut sorted = x.clone();
                sorted.sort();
                assert_eq!(sorted, v(), "nothing lost or duplicated");
            }
        }
    }

    #[test]
    fn finder_order_keeps_the_selection_order_and_drops_repeats() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(finder_order(&[p("/b.pdf"), p("/a.pdf"), p("/b.pdf"), p("/c.pdf")]), [p("/b.pdf"), p("/a.pdf"), p("/c.pdf")]);
        assert!(finder_order(&[]).is_empty());
    }
}
