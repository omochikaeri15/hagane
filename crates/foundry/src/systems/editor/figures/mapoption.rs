use super::resolved::{Choice, Rule};

pub const NOTICE: &str = "One row per map, found by its global Map ID\nCrown strength is the enemy magnification at that crown (in percent)\nA fixed cost multiplier above 0 replaces every unit's chapter cost";

pub(super) const CROWNS: &[Choice] = &[
    Choice::new(1, "1 Crown"),
    Choice::new(2, "2 Crowns"),
    Choice::new(3, "3 Crowns"),
    Choice::new(4, "4 Crowns"),
];

const RESETS: &[Choice] = &[
    Choice::new(0, "Never"),
    Choice::new(1, "Rewards"),
    Choice::new(2, "Rewards & Clears"),
    Choice::new(3, "Clear Limit"),
];

pub(super) const NAMES: &[(&str, &str)] = &[
    ("map_id", "Map ID"),
    ("max_crowns", "Crowns"),
    ("has_abyss", "Back Crowns"),
    ("crown_1_mag", "Crown 1 Strength %"),
    ("crown_2_mag", "Crown 2 Strength %"),
    ("crown_3_mag", "Crown 3 Strength %"),
    ("crown_4_mag", "Crown 4 Strength %"),
    ("guerrilla_set", "Guerrilla Schedule"),
    ("reset_type", "Reset"),
    ("max_clears", "Max Clears"),
    ("display_order", "Display Order"),
    ("cooldown_minutes", "Cooldown Minutes"),
    ("challenge_flag", "Records Entries"),
    ("difficulty", "Difficulty"),
    ("hidden_upon_clear", "Hide After Clear"),
    ("double_xp_ad", "Double XP Ad"),
    ("cost_multiplier", "Fixed Cost Multiplier"),
    ("energy_flag", "Costs Energy"),
    ("stage_display_setting", "Conditional Stage List"),
    ("cap_extension", "Cap Extension"),
];

pub fn rule(field: &str) -> Option<Rule> {
    match field {
        "map_id" => Some(Rule::Opaque),

        "max_crowns" => Some(Rule::Choice(CROWNS)),
        "reset_type" => Some(Rule::Choice(RESETS)),

        "challenge_flag" | "hidden_upon_clear" | "double_xp_ad" | "energy_flag" | "stage_display_setting" => {
            Some(Rule::Flag)
        }

        "has_abyss" | "crown_1_mag" | "crown_2_mag" | "crown_3_mag" | "crown_4_mag" | "guerrilla_set"
        | "max_clears" | "display_order" | "cooldown_minutes" | "difficulty" | "cost_multiplier"
        | "cap_extension" => Some(Rule::Floor(0)),

        _ => None,
    }
}
