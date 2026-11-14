use super::resolved::Rule;

pub const NOTICE: &str = "One row per item shop slot\nThe price is in Cat Food\nThe category is the text key the shop tab shows";

pub const LABELS: [&str; 7] =
    ["Shop Slot", "Item ID", "Quantity", "Price (Cat Food)", "Shows Quantity", "Category Text Key", "Icon"];

pub fn rule(index: usize) -> Rule {
    match index {
        0 | 5 => Rule::Opaque,
        4 => Rule::Flag,
        _ => Rule::Floor(0),
    }
}
