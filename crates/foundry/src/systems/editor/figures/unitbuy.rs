use super::resolved::{Choice, Rule};

pub fn rule(field: &str) -> Option<Rule> {
    match field {
        "rarity" => Some(Rule::Choice(
            &const {
                [
                    Choice::new(0, "N"),
                    Choice::new(1, "EX"),
                    Choice::new(2, "RR"),
                    Choice::new(3, "SR"),
                    Choice::new(4, "UR"),
                    Choice::new(5, "LR"),
                ]
            },
        )),
        "chapter_unlock_requirement" => Some(Rule::Offset(1)),

        "guide_order"
        | "true_form_obtain_level"
        | "true_form_unlock_level"
        | "ultra_form_unlock_level"
        | "level_cap_standard"
        | "version_added"
        | "egg_id_normal"
        | "egg_id_evolved" => Some(Rule::Floor(-1)),

        "currency_type"
        | "stage_unlock_requirement"
        | "purchase_cost"
        | "upgrade_cost_1"
        | "upgrade_cost_2"
        | "upgrade_cost_3"
        | "upgrade_cost_4"
        | "upgrade_cost_5"
        | "upgrade_cost_6"
        | "upgrade_cost_7"
        | "upgrade_cost_8"
        | "upgrade_cost_9"
        | "upgrade_cost_10"
        | "sell_xp_yield"
        | "level_cap_ch2"
        | "base_max_plus_level"
        | "level_cap_ch1"
        | "true_form_id"
        | "ultra_form_id"
        | "true_form_xp_cost"
        | "true_form_material_1_id"
        | "true_form_material_1_quantity"
        | "true_form_material_2_id"
        | "true_form_material_2_quantity"
        | "true_form_material_3_id"
        | "true_form_material_3_quantity"
        | "true_form_material_4_id"
        | "true_form_material_4_quantity"
        | "true_form_material_5_id"
        | "true_form_material_5_quantity"
        | "ultra_form_xp_cost"
        | "ultra_form_material_1_id"
        | "ultra_form_material_1_quantity"
        | "ultra_form_material_2_id"
        | "ultra_form_material_2_quantity"
        | "ultra_form_material_3_id"
        | "ultra_form_material_3_quantity"
        | "ultra_form_material_4_id"
        | "ultra_form_material_4_quantity"
        | "ultra_form_material_5_id"
        | "ultra_form_material_5_quantity"
        | "level_cap_catseye"
        | "level_cap_plus"
        | "normal_evolution_y_offset"
        | "evolved_evolution_y_offset"
        | "true_evolution_y_offset"
        | "ultra_evolution_y_offset"
        | "sell_np_yield"
        | "fanfare_tier"
        | "evolved_form_unlock_level"
        | "true_form_event_id"
        | "plus_cap_grows" | "item_powerup_group" => Some(Rule::Plain),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::rule;
    use crate::systems::editor::figures::{schema, Subject};

    #[test]
    fn every_buy_column_has_a_rule() {
        for entry in schema::of(Subject::Buy).order() {
            assert!(
                rule(entry.field).is_some(),
                "unitbuy: codex publishes {}, which no resolved rule arm names",
                entry.field,
            );
        }
    }
}
