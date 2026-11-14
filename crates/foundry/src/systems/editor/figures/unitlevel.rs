use super::resolved::Rule;

pub const NOTICE: &str =
    "Each bracket is the percent of the base stat added per level inside it\nThe last one keeps going past its own range";

pub fn rule() -> Rule {
    Rule::Plain
}
