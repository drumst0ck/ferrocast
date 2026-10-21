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

/// The slot (gap before row `k`, `rows.len()` = after the last) a pointer coordinate `p` falls in,
/// given each row's `(start, end)` extent along the list axis: the first row whose midpoint is
/// still ahead of the pointer. Above the first row is 0, below the last is `rows.len()`.
pub fn slot_for_pointer(rows: &[(f32, f32)], p: f32) -> usize {
    rows.iter().position(|(a, b)| p < (a + b) / 2.0).unwrap_or(rows.len())
}

/// The slot (gap before cell `k`, `cells.len()` = after the last) a pointer `p = (x, y)` falls in,
/// for cells laid out in reading order and wrapping onto rows; each cell is `(x0, y0, x1, y1)`.
/// Above the first row is 0, below the last row or past the last cell is `cells.len()`. Between
/// two rows the nearer row wins; inside a row the slot is the gap before the first cell whose
/// midpoint is still ahead of the pointer, or the gap before the next row's first cell when the
/// pointer is past the row's end. A single row gives the same answer as [`slot_for_pointer`].
pub fn slot_for_grid_pointer(cells: &[(f32, f32, f32, f32)], p: (f32, f32)) -> usize {
    let n = cells.len();
    if n == 0 {
        return 0;
    }
    if p.0.is_nan() || p.1.is_nan() {
        return n;
    }
    // rows as (first cell, one past the last cell, top, bottom): a cell starts a new row when it
    // begins below the bottom of the row so far
    let mut rows: Vec<(usize, usize, f32, f32)> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        match rows.last_mut() {
            Some(r) if c.1 < r.3 => {
                r.1 = i + 1;
                r.2 = r.2.min(c.1);
                r.3 = r.3.max(c.3);
            }
            _ => rows.push((i, i + 1, c.1, c.3)),
        }
    }
    if p.1 < rows[0].2 {
        return 0;
    }
    let mut row = None;
    for (k, r) in rows.iter().enumerate() {
        if p.1 <= r.3 {
            row = Some(k);
            break;
        }
        if let Some(next) = rows.get(k + 1) {
            if p.1 < next.2 {
                row = Some(if p.1 < (r.3 + next.2) / 2.0 { k } else { k + 1 });
                break;
            }
        }
    }
    let Some(k) = row else { return n };
    let (start, end, ..) = rows[k];
    cells[start..end].iter().position(|c| p.0 < (c.0 + c.2) / 2.0).map_or(end, |i| start + i)
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

    #[test]
    fn slot_for_pointer_uses_row_midpoints() {
        let rows = [(0.0, 40.0), (44.0, 84.0), (88.0, 128.0)];
        assert_eq!(slot_for_pointer(&rows, -50.0), 0, "above the first row");
        assert_eq!(slot_for_pointer(&rows, 19.9), 0);
        assert_eq!(slot_for_pointer(&rows, 20.0), 1, "at the midpoint the pointer is past the row");
        assert_eq!(slot_for_pointer(&rows, 70.0), 2);
        assert_eq!(slot_for_pointer(&rows, 108.0), 3);
        assert_eq!(slot_for_pointer(&rows, 900.0), 3, "below the last row is the end slot");
        assert_eq!(slot_for_pointer(&[], 5.0), 0, "empty list");
        assert_eq!(slot_for_pointer(&[(0.0, 10.0)], f32::NAN), 1, "NaN never panics");
    }

    /// Two rows of three 40x60 cells with 4 px gaps: row 0 at y 0..60, row 1 at y 70..130.
    fn grid() -> Vec<(f32, f32, f32, f32)> {
        let mut c = vec![];
        for row in 0..2 {
            for col in 0..3 {
                let (x, y) = (col as f32 * 44.0, row as f32 * 70.0);
                c.push((x, y, x + 40.0, y + 60.0));
            }
        }
        c
    }

    #[test]
    fn slot_for_grid_pointer_follows_reading_order() {
        let g = grid();
        assert_eq!(slot_for_grid_pointer(&g, (50.0, -30.0)), 0, "above the first row is the first slot");
        assert_eq!(slot_for_grid_pointer(&g, (5.0, 30.0)), 0, "left half of the first cell");
        assert_eq!(slot_for_grid_pointer(&g, (25.0, 30.0)), 1, "right half of a cell is the gap after it");
        assert_eq!(slot_for_grid_pointer(&g, (50.0, 30.0)), 1);
        assert_eq!(slot_for_grid_pointer(&g, (70.0, 30.0)), 2);
        assert_eq!(slot_for_grid_pointer(&g, (500.0, 30.0)), 3, "past the end of a row is the gap before the next row's first cell");
        assert_eq!(slot_for_grid_pointer(&g, (5.0, 100.0)), 3, "second row, left half");
        assert_eq!(slot_for_grid_pointer(&g, (60.0, 100.0)), 4);
        assert_eq!(slot_for_grid_pointer(&g, (500.0, 100.0)), 6, "past the last cell is the end slot");
        assert_eq!(slot_for_grid_pointer(&g, (5.0, 900.0)), 6, "below the last row is the end slot");
    }

    #[test]
    fn slot_for_grid_pointer_between_rows_picks_the_nearer_row() {
        let g = grid();
        // the gap is y 60..70, its middle 65
        assert_eq!(slot_for_grid_pointer(&g, (5.0, 62.0)), 0, "closer to row 0");
        assert_eq!(slot_for_grid_pointer(&g, (5.0, 68.0)), 3, "closer to row 1");
        assert_eq!(slot_for_grid_pointer(&g, (500.0, 62.0)), 3);
    }

    #[test]
    fn slot_for_grid_pointer_matches_the_row_version_for_a_single_row() {
        let cells: Vec<_> = (0..4).map(|i| (i as f32 * 44.0, 0.0, i as f32 * 44.0 + 40.0, 60.0)).collect();
        let spans: Vec<_> = cells.iter().map(|c| (c.0, c.2)).collect();
        for x in [-10.0, 0.0, 19.9, 20.0, 60.0, 100.0, 150.0, 400.0] {
            assert_eq!(slot_for_grid_pointer(&cells, (x, 30.0)), slot_for_pointer(&spans, x), "x={x}");
        }
    }

    #[test]
    fn slot_for_grid_pointer_empty_and_nan_never_panic() {
        assert_eq!(slot_for_grid_pointer(&[], (5.0, 5.0)), 0, "empty grid");
        let g = grid();
        assert_eq!(slot_for_grid_pointer(&g, (f32::NAN, 30.0)), 6);
        assert_eq!(slot_for_grid_pointer(&g, (5.0, f32::NAN)), 6);
        assert_eq!(slot_for_grid_pointer(&[(0.0, 0.0, 10.0, 10.0)], (50.0, 5.0)), 1, "one cell, past it");
    }

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
