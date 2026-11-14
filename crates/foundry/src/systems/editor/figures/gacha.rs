use super::resolved::Rule;

pub const NOTICE: &str = "How a capsule set looks (its banner, ticket, animation and images)\nA short row reads its missing columns as unset";

pub const LABELS: [&str; 11] = [
    "Set ID",
    "Banner Shown",
    "Ticket Item ID",
    "Capsule Animation",
    "Button Image",
    "Series ID",
    "Menu Image",
    "Featured Cat",
    "Idle Animation",
    "Image ID",
    "Get Count",
];

const BANNER: usize = 1;

pub(super) const SERIES: usize = 5;

const IDLE: usize = 8;

pub(super) const COUNT: usize = 10;

pub fn rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        BANNER | IDLE => Rule::Flag,
        SERIES | COUNT => Rule::Floor(0),
        _ => Rule::Floor(-1),
    }
}

pub(super) fn fallback(index: usize) -> i32 {
    match index {
        0 | BANNER | SERIES | IDLE | COUNT => 0,
        _ => -1,
    }
}
