use super::resolved::Rule;

pub const ITEM_ID_COLUMN: usize = 3;

pub const NOTICE: &str = "One row per catalog item, shared by every stage that drops it\nThe row's position below the header is the line its name sits on in GatyaitemName";

pub fn rule(field: &str) -> Option<Rule> {
    match field {
        "reflect_or_storage" => Some(Rule::Flag),

        "stage_drop_item_id" | "sever_id" | "src_item_id" | "gatya_ticket_id" | "img_id" => {
            Some(Rule::Floor(-1))
        }

        "rarity" | "price" | "quantity" | "category" | "index" | "main_menu_type" => Some(Rule::Floor(0)),

        _ => None,
    }
}
