use super::resolved::Rule;

pub const NOTICE: &str = "An Officers' Club reward\nThe item count is how many of the item triplets after it are read";

pub const LABELS: [&str; 13] = [
    "Club Item ID",
    "Lower Limit",
    "Upper Limit",
    "Item Count",
    "Item 1 Category",
    "Item 1 ID",
    "Item 1 Value",
    "Item 2 Category",
    "Item 2 ID",
    "Item 2 Value",
    "Item 3 Category",
    "Item 3 ID",
    "Item 3 Value",
];

const ITEM_COUNT: usize = 3;

const MAX_ITEMS: i32 = 3;

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        ITEM_COUNT => Rule::Range(0, MAX_ITEMS),
        _ => Rule::Floor(0),
    }
}
