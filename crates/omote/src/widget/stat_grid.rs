use dioxus::prelude::*;

use super::{Superscript, Tip};

const CELL_TEXT_SIZE: f32 = 13.0;
const CELL_WIDTH: f32 = 82.0;
const CELL_GAP: f32 = 4.0;

pub(crate) fn span(cells: usize) -> f32 {
    cells as f32 * CELL_WIDTH + cells.saturating_sub(1) as f32 * CELL_GAP
}

#[component]
pub(crate) fn StatHeader(label: String) -> Element {
    rsx! {
        div { class: "stat-cell header", "{label}" }
    }
}

#[component]
pub(crate) fn StatValue(header: String, value: String) -> Element {
    rsx! {
        Tip { text: "{header}: {value}", touch: true,
            div { class: "stat-cell value", title: "{header}: {value}", "{value}" }
        }
    }
}

#[component]
pub(crate) fn StatFrames(header: String, frames: i32) -> Element {
    let seconds = frames as f32 / 30.0;
    let raw = format!("{seconds:.2}s^{frames}f");

    rsx! {
        Tip { text: "{header}: {raw}", touch: true,
            div { class: "stat-cell value", title: "{header}: {raw}",
                Superscript { text: raw.clone(), size: CELL_TEXT_SIZE }
            }
        }
    }
}
