use strsim::jaro_winkler;

const EXACT: u32 = 10_000;
const PREFIX: u32 = 9_000;
const WORDS: u32 = 8_000;
const INSIDE: u32 = 7_000;
pub(crate) const CLOSENESS: u32 = 86;
const WHOLE: f64 = 100.0;
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
        self.score_within(name, CLOSENESS)
    }

    pub(crate) fn score_within(&self, name: &str, closeness: u32) -> Option<u32> {
        let held = normalized(name);
        let floor = f64::from(closeness) / WHOLE;
        let lenient = closeness <= CLOSENESS;

        if self.joined.is_empty() || held.is_empty() {
            return None;
        }

        let shorter = |base: u32| base.saturating_sub(u32::try_from(held.chars().count().abs_diff(self.joined.chars().count())).unwrap_or(u32::MAX).min(999));

        if held == self.joined {
            return Some(EXACT);
        }

        let name_words = words(name);
        let ranked = if held.starts_with(&self.joined) {
            Some(shorter(PREFIX))
        } else if !self.words.is_empty() && self.words.iter().all(|wanted| name_words.iter().any(|word| word.starts_with(wanted.as_str()))) {
            Some(shorter(WORDS))
        } else if held.contains(&self.joined) {
            Some(shorter(INSIDE))
        } else {
            None
        };

        if lenient && ranked.is_some() {
            return ranked;
        }

        let whole = jaro_winkler(&self.joined, &held);
        let per_word = if self.words.is_empty() {
            0.0
        } else {
            self.words.iter().map(|wanted| name_words.iter().map(|word| jaro_winkler(wanted, word)).fold(0.0, f64::max)).sum::<f64>() / self.words.len() as f64
        };
        let similar = whole.max(per_word);

        (similar >= floor).then(|| ranked.unwrap_or((similar * SIMILAR_SCALE) as u32))
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
