use super::resolved::Rule;

pub const NOTICE: &str = "One row per battle background, found by its ID\nEach gradient goes from its top color to its bottom color\nFog only draws once its alpha is set";

const CHANNEL_FLOOR: i32 = 0;

const CHANNEL_CEILING: i32 = 255;

pub(super) const NAMES: &[(&str, &str)] = &[
    ("id", "Background ID"),
    ("model_id", "Uses Model Cuts"),
    ("has_upper_layer", "Draws Upper Layer"),
    ("image_id", "Image ID"),
];

pub fn rule(field: &str) -> Option<Rule> {
    match field {
        "id" => Some(Rule::Opaque),
        "has_upper_layer" => Some(Rule::Flag),
        "model_id" => Some(Rule::Floor(0)),
        "image_id" => Some(Rule::Floor(-1)),

        "sky_top_red" | "sky_top_green" | "sky_top_blue" | "sky_bottom_red" | "sky_bottom_green"
        | "sky_bottom_blue" | "ground_top_red" | "ground_top_green" | "ground_top_blue"
        | "ground_bottom_red" | "ground_bottom_green" | "ground_bottom_blue" | "fog_top_red"
        | "fog_top_green" | "fog_top_blue" | "fog_top_alpha" | "fog_bottom_red" | "fog_bottom_green"
        | "fog_bottom_blue" | "fog_bottom_alpha" => Some(Rule::Range(CHANNEL_FLOOR, CHANNEL_CEILING)),

        _ => None,
    }
}
