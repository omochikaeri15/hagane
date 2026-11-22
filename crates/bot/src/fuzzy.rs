use strsim::jaro_winkler;

const EXACT: u32 = 10_000;
const PREFIX: u32 = 9_000;
const WORDS: u32 = 8_000;
const INSIDE: u32 = 7_000;
const SIMILAR_FLOOR: f64 = 0.86;
const SIMILAR_SCALE: f64 = 6_000.0;

pub(crate) fn strong(score: u32) -> bool {
    f64::from(score) > SIMILAR_SCALE
}

pub(crate) fn normalized(text: &str) -> String {
    text.chars().filter(|glyph| glyph.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn words(text: &str) -> Vec<String> {
    text.split(|glyph: char| !glyph.is_alphanumeric()).filter(|word| !word.is_empty()).map(normalized).collect()
}

pub(crate) struct Query {
    joined: String,
    words: Vec<String>,
}

impl Query {
    pub(crate) fn new(text: &str) -> Self {
        Self { joined: normalized(text), words: words(text) }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.joined.is_empty()
    }

    pub(crate) fn score(&self, name: &str) -> Option<u32> {
        let held = normalized(name);

        if self.joined.is_empty() || held.is_empty() {
            return None;
        }

        let shorter = |base: u32| base.saturating_sub(u32::try_from(held.chars().count().abs_diff(self.joined.chars().count())).unwrap_or(u32::MAX).min(999));

        if held == self.joined {
            return Some(EXACT);
        }

        if held.starts_with(&self.joined) {
            return Some(shorter(PREFIX));
        }

        let name_words = words(name);

        if !self.words.is_empty() && self.words.iter().all(|wanted| name_words.iter().any(|word| word.starts_with(wanted.as_str()))) {
            return Some(shorter(WORDS));
        }

        if held.contains(&self.joined) {
            return Some(shorter(INSIDE));
        }

        let whole = jaro_winkler(&self.joined, &held);
        let per_word = if self.words.is_empty() {
            0.0
        } else {
            self.words.iter().map(|wanted| name_words.iter().map(|word| jaro_winkler(wanted, word)).fold(0.0, f64::max)).sum::<f64>() / self.words.len() as f64
        };
        let similar = whole.max(per_word);

        (similar >= SIMILAR_FLOOR).then_some((similar * SIMILAR_SCALE) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closer_matches_rank_higher() {
        let query = Query::new("jamiera");
        let exact = Query::new("jamiera cat").score("Jamiera Cat");
        let prefix = query.score("Jamiera Cat");
        let inside = Query::new("miera").score("Jamiera Cat");

        assert!(exact > prefix);
        assert!(prefix > inside);
    }

    // People misspell names; a close typo should still land.
    #[test]
    fn a_typo_still_matches() {
        assert!(Query::new("jameira").score("Jamiera Cat").is_some());
        assert!(Query::new("bahamut").score("Crazed Bahamut Cat").is_some());
        assert!(Query::new("zzzzqq").score("Jamiera Cat").is_none());
    }
}
