use super::resolved::Rule;
use super::schema::BRACKET;

pub const NOTICE: &str = "One value per ten-level bracket, in the same brackets as this cat's level curve\nThey set this cat's level-up prices in XP";

pub const BRACKETS: usize = 20;

pub(super) fn label(index: usize) -> String {
    format!("Levels {}-{}", index * BRACKET + 1, (index + 1) * BRACKET)
}

pub fn rule() -> Rule {
    Rule::Floor(0)
}
