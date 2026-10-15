//! Exact-count suppression never drops a later user copy just because it is fast.
pub fn should_record(count: isize, last: isize, own_write: isize) -> bool {
    count != last && count != own_write
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_write_is_skipped_but_next_user_copy_is_recorded() {
        assert!(!should_record(12, 11, 12));
        assert!(should_record(13, 12, 12));
        assert!(!should_record(13, 13, 12));
    }
}
