use serde::{Deserialize, Serialize};

const JAPANESE: &str = "ja";

const MARK_LEAD: u8 = 0xEF;

const COMMENT: &[u8] = b"//";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Separator {
    Pipe,
    Tab,
    Comma,
}

impl Separator {
    pub const ALL: [Self; 3] = [Self::Pipe, Self::Tab, Self::Comma];

    pub fn detect(text: &str) -> Option<Self> {
        if text.contains('|') {
            return Some(Self::Pipe);
        }

        text.chars().find_map(|current| match current {
            '\t' => Some(Self::Tab),
            ',' => Some(Self::Comma),
            _ => None,
        })
    }

    pub fn localized(name: &str) -> Self {
        let stem = name.rsplit_once('.').map_or(name, |(head, _)| head);
        let japanese = stem.rsplit_once('_').is_some_and(|(_, code)| code == JAPANESE);

        if japanese { Self::Comma } else { Self::Pipe }
    }

    pub const fn char(self) -> char {
        match self {
            Self::Pipe => '|',
            Self::Tab => '\t',
            Self::Comma => ',',
        }
    }

    pub fn split(self, line: &str) -> impl Iterator<Item = &str> {
        line.split(self.char())
    }
}

pub(crate) fn resolve(separator: Option<Separator>, text: &str) -> char {
    separator
        .or_else(|| Separator::detect(text))
        .unwrap_or(Separator::Comma)
        .char()
}

pub fn scrub(bytes: &[u8]) -> String {
    let raw_text = String::from_utf8_lossy(bytes);
    let mut clean_text = String::with_capacity(raw_text.len());
    let mut rest: &str = &raw_text;

    while let Some(at) = rest.bytes().position(|byte| matches!(byte, b'\r' | b'\0' | MARK_LEAD)) {
        let (kept, tail) = rest.split_at(at);
        let mut chars = tail.chars();
        let current = chars.next();

        clean_text.push_str(kept);
        rest = chars.as_str();

        match current {
            Some('\r') => {
                clean_text.push('\n');
                rest = rest.strip_prefix('\n').unwrap_or(rest);
            }
            Some('\u{feff}' | '\0') | None => {}
            Some(other) => clean_text.push(other),
        }
    }

    clean_text.push_str(rest);
    clean_text
}

pub(crate) fn uncommented(line: &str) -> &str {
    line.as_bytes().windows(COMMENT.len()).position(|pair| pair == COMMENT).map_or(line, |at| &line[..at])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakHandling {
    Space,
    Newline,
    Delete,
}

pub fn strip_html_tags(input: &str, handling: BreakHandling) -> String {
    let mut stripped = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find('<') {
        stripped.push_str(&rest[..start]);
        rest = &rest[start..];

        let Some(end) = rest.find('>') else {
            break;
        };

        let tag = &rest[..=end];
        rest = &rest[end + 1..];

        if tag.len() < 3 || !tag[..3].eq_ignore_ascii_case("<br") {
            continue;
        }

        match handling {
            BreakHandling::Space => {
                if !stripped.ends_with(' ') && !rest.starts_with(' ') {
                    stripped.push(' ');
                }
            }
            BreakHandling::Newline => stripped.push('\n'),
            BreakHandling::Delete => continue,
        }
    }

    stripped.push_str(rest);
    stripped
}

pub fn lookup(
    data: &[u8],
    key: &str,
    search_col: usize,
    target_col: usize,
    separator: Option<Separator>,
) -> Option<String> {
    let content = scrub(data);
    let separator = resolve(separator, &content);

    for line in content.lines() {
        let Some(clean_line) = line.split("//").next() else {
            continue;
        };

        let trimmed_line = clean_line.trim();

        if trimmed_line.is_empty() {
            continue;
        }

        let Some(current_key) = trimmed_line.split(separator).nth(search_col) else {
            continue;
        };

        if current_key.trim() != key {
            continue;
        }

        let Some(target_value) = trimmed_line.split(separator).nth(target_col) else {
            continue;
        };

        return Some(target_value.trim().to_string());
    }

    None
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_anywhere_outranks_an_earlier_comma() {
        assert_eq!(Separator::detect("Soracte, mentor mental|text"), Some(Separator::Pipe));
        assert_eq!(Separator::detect("a\tb,c"), Some(Separator::Tab));
        assert_eq!(Separator::detect("a,b\tc"), Some(Separator::Comma));
        assert_eq!(Separator::detect("one whole name"), None);
    }

    #[test]
    fn a_stated_delimiter_overrides_the_content() {
        let shredded = "Soracte, mentor mental|Un mentor.";
        assert_eq!(Separator::Pipe.split(shredded).count(), 2);
        assert_eq!(Separator::detect(shredded).map(Separator::char), Some('|'));
        assert_eq!(resolve(Some(Separator::Comma), shredded), ',');
        assert_eq!(resolve(None, shredded), '|');
    }

    #[test]
    fn only_a_ja_suffix_names_a_comma_separated_table() {
        assert_eq!(Separator::localized("Unit_Explanation_ja.csv"), Separator::Comma);
        assert_eq!(Separator::localized("Unit_Explanation_en.csv"), Separator::Pipe);
        assert_eq!(Separator::localized("Unit_Explanation_th.csv"), Separator::Pipe);
        // A localized table shipped with no suffix at all is still bar-separated.
        assert_eq!(Separator::localized("matatabi_Popup.csv"), Separator::Pipe);
        assert_eq!(Separator::localized("TitleMessage"), Separator::Pipe);
        // "ja" has to be the suffix, not merely the tail of a longer word.
        assert_eq!(Separator::localized("jinja_level.csv"), Separator::Pipe);
    }

    #[test]
    fn detection_falls_back_to_a_comma() {
        assert_eq!(resolve(None, "one whole name"), ',');
        assert_eq!(Separator::ALL.map(Separator::char), ['|', '\t', ',']);
    }

    #[test]
    fn scrubbing_leaves_one_newline_per_break_and_no_marks() {
        assert_eq!(scrub(b"\xef\xbb\xbfa,b\r\nc\rd\n\0e\r\r\n"), "a,b\nc\nd\ne\n\n");
        // a fullwidth bang shares the mark's lead byte and has to survive
        assert_eq!(scrub("\u{ff01}\u{feff}x\r\u{feff}\n".as_bytes()), "\u{ff01}x\n\n");
        assert_eq!(scrub(b"a\xffb\xef"), "a\u{fffd}b\u{fffd}");
    }

    #[test]
    fn a_comment_starts_at_the_first_double_slash() {
        assert_eq!(uncommented("1,2//note//more"), "1,2");
        assert_eq!(uncommented("1/2,3/"), "1/2,3/");
        assert_eq!(uncommented("\u{30cd}\u{30b3}//x"), "\u{30cd}\u{30b3}");
        assert_eq!(uncommented("//"), "");
    }

    #[test]
    fn lookup_reads_the_column_of_the_stated_delimiter() {
        let table = b"1|Soracte, mentor mental|Un mentor.";
        assert_eq!(
            lookup(table, "1", 0, 1, Some(Separator::Pipe)).as_deref(),
            Some("Soracte, mentor mental"),
        );
    }
}
