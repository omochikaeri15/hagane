use super::resolved::Rule;

pub const NOTICE: &str = "What a Cat Base recipe needs before it unlocks\nAny requirement set to -1 is skipped";

pub const LABELS: [&str; 7] =
    ["Unlock ID", "Recipe", "Development Level", "Cleared Stage", "User Rank", "Cannon Attack Level", "Cannon Charge Level"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(-1),
    }
}
