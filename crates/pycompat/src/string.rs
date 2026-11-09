//! Python `str` methods: `isspace`, `isprintable`, `strip`, `rstrip` and `splitlines`.

/// `str.isspace` for one character, which is also what `\s` matches in a `str` regex.
/// Unicode White_Space plus the four ASCII separators U+001C..U+001F.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`.
pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `str.rstrip()`.
pub fn rstrip(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

/// `str.isprintable()` for one character.
///
/// ceiling: Python excludes every code point in categories Cc, Cf, Cs, Co, Cn, Zl, Zp, and Zs but the
/// space; without Unicode tables this covers controls, the separators, private use, and the common format
/// characters, so an unassigned or rare format code point prints raw where Python escapes it.
pub fn isprintable(c: char) -> bool {
    let code = c as u32;
    let separator = c != ' ' && c.is_whitespace();
    let format = matches!(code, 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x180e | 0x200b..=0x200f
        | 0x202a..=0x202e | 0x2060..=0x2064 | 0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb);
    let private = matches!(code, 0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd);
    !(c.is_control() || separator || format || private)
}

/// Whether `str.splitlines` breaks a line at `c`.
fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\x0b'
            | '\x0c'
            | '\x1c'
            | '\x1d'
            | '\x1e'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// `str.splitlines()`: lines without their breaks; `\r\n` is one break, and a final break adds no line.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    while let Some(offset) = text[start..].find(is_line_break) {
        let end = start + offset;
        lines.push(&text[start..end]);
        start = end + line_break_len(&text[end..]);
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// The bytes of the line break that `rest` starts with: `\r\n` is one break.
fn line_break_len(rest: &str) -> usize {
    if rest.starts_with("\r\n") {
        return 2;
    }
    rest.chars()
        .next()
        .expect("rest starts at a line break")
        .len_utf8()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitlines_matches_python() {
        assert_eq!(
            splitlines("a\nb\r\nc\rd\x0be\u{2028}f\n"),
            ["a", "b", "c", "d", "e", "f"]
        );
        assert_eq!(splitlines("a\n\nb"), ["a", "", "b"]);
        assert_eq!(splitlines("a\n\n"), ["a", ""]);
        assert_eq!(splitlines("a\r\nb\rc\u{2028}d\u{c}"), ["a", "b", "c", "d"]);
        assert_eq!(splitlines("a\u{1f}b"), ["a\u{1f}b"]);
        assert!(splitlines("").is_empty());
        assert_eq!(splitlines("\n"), [""]);
        assert_eq!(splitlines("\u{2028}x"), ["", "x"]);
    }

    #[test]
    fn splitlines_pairs_only_cr_with_a_following_lf() {
        assert_eq!(splitlines("a\r"), ["a"]);
        assert_eq!(splitlines("a\r\n"), ["a"]);
        assert_eq!(splitlines("\r\n\r\n"), ["", ""]);
        assert_eq!(splitlines("\r\r\n"), ["", ""]);
        assert_eq!(splitlines("\n\r"), ["", ""]);
        assert_eq!(splitlines("a\u{85}b\u{2029}"), ["a", "b"]);
        assert_eq!(splitlines("\r\nx"), ["", "x"]);
    }

    #[test]
    #[should_panic(expected = "rest starts at a line break")]
    fn a_line_break_length_needs_a_break_to_measure() {
        line_break_len("");
    }

    #[test]
    fn space_and_strip_match_python() {
        assert!(is_space('\u{1f}') && is_space('\u{a0}') && !is_space('\u{200b}'));
        assert_eq!(strip("\u{1c} a \u{3000}"), "a");
        assert_eq!(rstrip("\u{1c} a \t"), "\u{1c} a");
    }

    #[test]
    fn printable_matches_python() {
        assert!(isprintable(' ') && isprintable('é') && isprintable('a'));
        assert!(!isprintable('\u{1}') && !isprintable('\u{a0}') && !isprintable('\u{ad}'));
        assert!(!isprintable('\u{200b}') && !isprintable('\u{e000}'));
    }
}
