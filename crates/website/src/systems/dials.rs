use dioxus::prelude::*;

use codex::combat::Entity;
use foundry::systems::combat::lens::{self, Lens, MODES, ONE_HIT};

use crate::widget::Tip;

#[component]
pub(crate) fn Dials(stats: Entity, lens: Signal<Lens>) -> Element {
    let held = lens();
    let shown = lens::hits_label(lens::shown_hits(&stats, held.hits));
    let mode = lens::mode_label(held.mode);
    let choices: Vec<&'static str> = lens::hit_choices(&stats).into_iter().map(lens::hits_label).collect();

    rsx! {
        if lens::picks_hits(&stats) {
            select { class: "combo dial", value: "{shown}",
                onchange: move |event| lens.write().hits = lens::hits_of(&event.value()),
                for choice in choices {
                    option { value: "{choice}", selected: choice == shown, "{choice}" }
                }
            }
        } else {
            Tip { text: ONE_HIT.to_owned(), touch: true,
                div { class: "combo dial idle", "{shown}" }
            }
        }
        select { class: "combo dial", value: "{mode}",
            onchange: move |event| lens.write().mode = lens::mode_named(&event.value()),
            for choice in MODES.map(lens::mode_label) {
                option { value: "{choice}", selected: choice == mode, "{choice}" }
            }
        }
    }
}
