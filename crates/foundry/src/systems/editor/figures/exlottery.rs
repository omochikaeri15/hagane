use super::resolved::Rule;

pub const NOTICE: &str = "One row per EX lottery group, in group order\nA stage's EX draws use these groups by their position";

pub const LABELS: [&str; 2] = ["EX Map ID", "EX Stage"];

pub fn rule() -> Rule {
    Rule::Floor(0)
}
