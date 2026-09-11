//! Char-boundary-safe string utilities.
//!
//! Byte-indexed slicing (`&s[..n]`) panics when `n` lands inside a multi-byte
//! UTF-8 character (emoji, CJK, accented text). Use these helpers for any
//! display truncation instead of slicing by byte index.

/// Truncate `s` to at most `max_chars` characters without splitting a
/// multi-byte UTF-8 character.
///
/// Returns a borrowed sub-slice, so it never allocates. If `s` has
/// `max_chars` or fewer characters, the whole string is returned.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_shorter_than_limit_is_unchanged() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn ascii_exactly_at_limit_is_unchanged() {
        assert_eq!(truncate_chars("hello", 5), "hello");
    }

    #[test]
    fn ascii_longer_than_limit_is_truncated() {
        assert_eq!(truncate_chars("hello world", 5), "hello");
    }

    #[test]
    fn emoji_is_not_split() {
        // Each emoji is 4 bytes; &s[..5] would panic mid-emoji.
        let s = "🎉🎉🎉";
        assert_eq!(truncate_chars(s, 1), "🎉");
        assert_eq!(truncate_chars(s, 2), "🎉🎉");
        assert_eq!(truncate_chars(s, 3), s);
        assert_eq!(truncate_chars(s, 4), s);
    }

    #[test]
    fn cjk_is_not_split() {
        // Each CJK char is 3 bytes; &s[..4] would panic mid-char.
        let s = "日本語テキスト";
        assert_eq!(truncate_chars(s, 3), "日本語");
        assert_eq!(truncate_chars(s, 100), s);
    }

    #[test]
    fn mixed_ascii_and_multibyte() {
        let s = "ab🚀cd語";
        assert_eq!(truncate_chars(s, 3), "ab🚀");
        assert_eq!(truncate_chars(s, 5), "ab🚀cd");
        assert_eq!(truncate_chars(s, 6), s);
    }

    #[test]
    fn zero_limit_returns_empty() {
        assert_eq!(truncate_chars("hello", 0), "");
        assert_eq!(truncate_chars("", 0), "");
    }

    #[test]
    fn empty_input_is_unchanged() {
        assert_eq!(truncate_chars("", 5), "");
    }
}
