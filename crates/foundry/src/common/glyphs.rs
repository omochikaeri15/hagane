pub const ARROW: char = '\u{2192}';

pub fn ideograph(glyph: char) -> bool {
    matches!(
        glyph,
        '\u{3005}'..='\u{3007}'
            | '\u{3040}'..='\u{30ff}'
            | '\u{31f0}'..='\u{31ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
            | '\u{ff66}'..='\u{ff9f}'
            | '\u{20000}'..='\u{2fa1f}'
    )
}

pub fn unspaced(glyph: char) -> bool {
    ideograph(glyph) || matches!(glyph, '\u{0e00}'..='\u{0eff}' | '\u{1000}'..='\u{109f}' | '\u{1780}'..='\u{17ff}')
}

pub fn breakable(text: &str) -> bool {
    text.split_whitespace().nth(1).is_some() || text.chars().any(unspaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_unspaced_glyph_is_enough_to_break_a_name() {
        assert!(breakable("\u{30cd}\u{30b3}\u{30e0}\u{30fc}\u{30c8}"));
        assert!(breakable("EX\u{732b}"));
        assert!(breakable("\u{0e41}\u{0e21}\u{0e27}"));
        assert!(breakable("Orbital Annihilator"));
    }

    // Korean spaces its words, so a lone Korean word shrinks like a lone English one.
    #[test]
    fn a_lone_spaced_word_still_shrinks_instead() {
        assert!(!breakable("ragnarokssssssssssssssssss"));
        assert!(!breakable("\u{ace0}\u{c591}\u{c774}"));
        assert!(!breakable(" Cat "));
        assert!(!breakable(""));
    }
}
