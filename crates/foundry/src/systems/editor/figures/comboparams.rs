use super::resolved::Rule;

pub const NOTICE: &str = "One row per combo effect\nEach column is how strong the effect is at that combo size (in the effect's own unit)\nDown is what a weakening combo applies";

pub const BANDS: [&str; 6] = ["Small", "Medium", "Large", "Ultimate", "Down", "Granted"];

pub fn rule() -> Rule {
    Rule::Plain
}
