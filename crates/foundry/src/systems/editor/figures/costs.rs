use super::resolved::Rule;
use super::schema::{COST_HEAD, COST_LEVELS};
use super::{Draft, Marks};

pub const NOTICE: &str =
    "Cost curves are shared by every unit\nDimmed IDs are ones this unit's talents never use";

pub fn references(used: Marks, id: u32) -> bool {
    used.iter().any(|cost| u32::from(*cost) == id)
}

pub fn unstored(draft: &Draft) -> Option<usize> {
    (draft.stored() < draft.len()).then(|| draft.stored())
}

pub fn rule(index: usize) -> Rule {
    match index.checked_sub(COST_HEAD) {
        None => Rule::Plain,
        Some(level) if level < COST_LEVELS => Rule::Floor(0),
        Some(_) => Rule::Opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::{rule, Rule, COST_HEAD, COST_LEVELS};

    // codex parses each cost with u16::from_str and drops what fails, so a negative
    // cost would not be stored as -1: it would vanish and shift every later level up one.
    #[test]
    fn every_level_refuses_a_cost_the_engine_would_drop() {
        for index in COST_HEAD..COST_HEAD + COST_LEVELS {
            assert_eq!(rule(index), Rule::Floor(0), "level column {index} must stay non-negative");
        }
    }

    #[test]
    fn a_column_past_the_tenth_level_is_flagged_rather_than_shown_as_a_cost() {
        assert_eq!(rule(COST_HEAD + COST_LEVELS), Rule::Opaque);
    }
}
