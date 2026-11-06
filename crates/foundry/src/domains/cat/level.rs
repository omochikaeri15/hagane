const JOIN: char = '+';

pub fn typable(text: &str) -> bool {
    text.chars().all(|glyph| glyph.is_ascii_digit() || glyph == JOIN)
}

pub fn split(text: &str, caps: Option<(u32, u32)>) -> (u32, u32) {
    let mut terms = text.split(JOIN);
    let base = terms.next().and_then(|term| term.trim().parse::<u32>().ok()).unwrap_or(0).max(1);
    let mut joined = false;
    let mut plus = 0u32;

    for term in terms {
        joined = true;
        plus = plus.saturating_add(term.trim().parse::<u32>().unwrap_or(0));
    }

    match caps {
        Some((top, extra)) if joined => (base.min(top), plus.min(extra)),
        Some((top, extra)) => {
            let total = base.min(top.saturating_add(extra));
            let kept = total.min(top);

            (kept, total - kept)
        }
        None => (base, plus),
    }
}

pub fn total(text: &str, caps: Option<(u32, u32)>) -> i32 {
    let (base, plus) = split(text, caps);

    i32::try_from(base.saturating_add(plus)).unwrap_or(i32::MAX)
}

pub fn within(level: i32, caps: Option<(u32, u32)>) -> i32 {
    let ceiling = caps.map_or(i32::MAX, |(top, extra)| i32::try_from(top.saturating_add(extra)).unwrap_or(i32::MAX));

    level.clamp(1, ceiling)
}

pub fn capped(text: &str, caps: Option<(u32, u32)>) -> String {
    let Some((top, extra)) = caps else {
        return text.to_owned();
    };
    let (first, rest) = text.split_once(JOIN).map_or((text, None), |(first, rest)| (first, Some(rest)));
    let base = first.trim().parse::<u32>().ok();
    let ceiling = top.saturating_add(extra);

    let Some(rest) = rest else {
        return match base {
            Some(level) if level > ceiling => ceiling.to_string(),
            _ => text.to_owned(),
        };
    };

    let plus = rest.split(JOIN).map(|term| term.trim().parse::<u32>().unwrap_or(0)).fold(0u32, u32::saturating_add);
    let base = match base {
        Some(level) if level > top => top.to_string(),
        _ => first.to_owned(),
    };
    let plus = if plus > extra { extra.to_string() } else { rest.to_owned() };

    format!("{base}{JOIN}{plus}")
}

#[cfg(test)]
mod tests {
    use super::{capped, split, total, within};

    const UBER: Option<(u32, u32)> = Some((60, 70));
    const FIXED: Option<(u32, u32)> = Some((1, 0));

    #[test]
    fn each_side_stops_at_its_own_cap() {
        assert_eq!(capped("70+90", UBER), "60+70");
        assert_eq!(split("70+90", UBER), (60, 70));
        // plus levels the unit cannot hold are dropped rather than moved over to the base
        assert_eq!(capped("30+90", UBER), "30+70");
        assert_eq!(total("30+90", UBER), 100);
        assert_eq!(capped("50+30+60", UBER), "50+70");
    }

    #[test]
    fn a_plain_number_is_a_total_filled_base_first() {
        assert_eq!(capped("500", UBER), "130");
        assert_eq!(split("90", UBER), (60, 30));
        assert_eq!(split("45", UBER), (45, 0));
    }

    #[test]
    fn text_still_being_typed_is_left_alone() {
        for text in ["", "5", "50+", "+7", "0", "60+70", "05+7"] {
            assert_eq!(capped(text, UBER), text);
        }

        assert_eq!(split("", UBER), (1, 0));
        assert_eq!(split("0+5", UBER), (1, 5));
    }

    #[test]
    fn a_unit_that_never_levels_stays_at_one() {
        assert_eq!(capped("50", FIXED), "1");
        assert_eq!(capped("50+70", FIXED), "1+0");
        assert_eq!(total("50+70", FIXED), 1);
    }

    #[test]
    fn a_single_level_lands_inside_the_units_range() {
        assert_eq!(within(50, FIXED), 1);
        assert_eq!(within(200, UBER), 130);
        assert_eq!(within(-3, UBER), 1);
        assert_eq!(within(200, None), 200);
    }

    #[test]
    fn without_a_row_nothing_is_capped() {
        assert_eq!(capped("200+300", None), "200+300");
        assert_eq!(split("200", None), (200, 0));
        assert_eq!(total("200+300", None), 500);
    }
}
