use std::ops::Range;

use unicode_width::UnicodeWidthStr;

/// Greedy word wrap as byte ranges, so a cursor can be placed in the wrapped text.
/// The spaces a row breaks at belong to neither row.
pub(in crate::tui) fn rows(text: &str, max: usize) -> Vec<Range<usize>> {
    let max = max.max(1);
    let mut rows = Vec::new();
    let mut base = 0;
    for paragraph in text.split('\n') {
        let (mut start, mut end, mut at) = (base, base, base);
        for word in paragraph.split(' ') {
            let stop = at + word.len();
            if text[start..stop].width() > max {
                if end > start {
                    rows.push(start..end);
                }
                start = at;
                for (offset, ch) in word.char_indices() {
                    let here = at + offset;
                    if here > start && text[start..here + ch.len_utf8()].width() > max {
                        rows.push(start..here);
                        start = here;
                    }
                }
            }
            end = stop;
            if word.is_empty() && start == end && start > base {
                start = (stop + 1).min(base + paragraph.len());
                end = start;
            }
            at = stop + 1;
        }
        rows.push(start..end);
        base += paragraph.len() + 1;
    }
    rows
}

/// The row and display column of a char cursor in the wrapped text.
pub(in crate::tui) fn locate(text: &str, cursor: usize, max: usize) -> (usize, usize) {
    let rows = rows(text, max);
    let byte = byte_at(text, cursor);
    let row = rows.iter().rposition(|row| row.start <= byte).unwrap_or(0);
    let range = &rows[row];
    (row, text[range.start..byte.min(range.end)].width())
}

/// The char cursor one row up or down at the same column, or None past the first or last row.
pub(in crate::tui) fn vertical(
    text: &str,
    cursor: usize,
    max: usize,
    delta: isize,
) -> Option<usize> {
    let (row, column) = locate(text, cursor, max);
    let rows = rows(text, max);
    let index = row.checked_add_signed(delta)?;
    let target = rows.get(index)?;
    let mut byte = target.start;
    for (offset, ch) in text[target.clone()].char_indices() {
        let next = target.start + offset + ch.len_utf8();
        if text[target.start..next].width() > column {
            break;
        }
        byte = next;
    }
    // A hard break ends where the next row starts, and a cursor there draws on the next row.
    if byte > target.start && rows.get(index + 1).is_some_and(|next| next.start == byte) {
        byte = text[..byte]
            .char_indices()
            .next_back()
            .map_or(target.start, |(at, _)| at);
    }
    Some(text[..byte].chars().count())
}

fn byte_at(text: &str, cursor: usize) -> usize {
    text.char_indices()
        .nth(cursor)
        .map_or(text.len(), |(at, _)| at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(text: &str, max: usize) -> Vec<&str> {
        rows(text, max).into_iter().map(|row| &text[row]).collect()
    }

    #[test]
    fn wraps_at_spaces_and_hard_breaks_long_words() {
        assert_eq!(shown("one two three", 7), ["one two", "three"]);
        assert_eq!(shown("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(shown("a\n\nb", 4), ["a", "", "b"]);
        assert_eq!(shown("  - item", 10), ["  - item"]);
        assert_eq!(shown("", 4), [""]);
        assert_eq!(shown("aaaa  bb", 4), ["aaaa", "bb"]);
        assert_eq!(shown("aaaa    bbbb", 4), ["aaaa", "bbbb"]);
        assert_eq!(shown("ab  cd", 10), ["ab  cd"]);
    }

    #[test]
    fn cursor_follows_the_wrapped_rows() {
        let text = "one two three";
        assert_eq!(locate(text, 0, 7), (0, 0));
        assert_eq!(locate(text, 7, 7), (0, 7));
        assert_eq!(locate(text, 10, 7), (1, 2));
        assert_eq!(locate("ab\ncd", 3, 10), (1, 0));
        assert_eq!(vertical(text, 10, 7, -1), Some(2));
        assert_eq!(vertical(text, 6, 7, 1), Some(13));
        assert_eq!(vertical(text, 2, 7, -1), None);
        assert_eq!(vertical("日本語\nab", 2, 10, 1), Some(6));
        assert_eq!(vertical("ab\n日本語", 1, 10, 1), Some(3));
        assert_eq!(
            vertical("abcdefgh", 8, 4, -1),
            Some(3),
            "stays off the hard break"
        );
        assert_eq!(locate("abcdefgh", 3, 4), (0, 3));
    }
}
