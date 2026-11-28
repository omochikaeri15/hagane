use dioxus::prelude::*;

use codex::combat::{Entity, Hits, Mode};
use foundry::systems::combat::lens::{self, Lens, MODES, ONE_HIT};

use crate::app;
use crate::platform::Platform;
use crate::widget::Tip;

#[component]
pub(crate) fn Dials(stats: Entity, lens: Signal<Lens>, versus: bool, on_mode: Option<EventHandler<()>>) -> Element {
    let held = lens();
    let shown = lens::hits_label(lens::shown_hits(&stats, held.hits));
    let mode = lens::mode_label(held.mode);
    let choices: Vec<&'static str> = lens::hit_choices(&stats).into_iter().map(lens::hits_label).collect();

    rsx! {
        if !lens::picks_hits(&stats) {
            Tip { text: ONE_HIT.to_owned(), touch: true,
                div { class: "combo dial idle", "{shown}" }
            }
        } else if !versus {
            div { class: "combo dial idle", {lens::hits_label(Hits::All)} }
        } else {
            select { class: "combo dial", value: "{shown}",
                onchange: move |event| lens.write().hits = lens::hits_of(&event.value()),
                for choice in choices {
                    option { value: "{choice}", selected: choice == shown, "{choice}" }
                }
            }
        }
        if versus {
            select { class: "combo dial", value: "{mode}",
                onchange: move |event| {
                    lens.write().mode = lens::mode_named(&event.value());

                    if let Some(on_mode) = on_mode {
                        on_mode.call(());
                    }
                },
                for choice in MODES.map(lens::mode_label) {
                    option { value: "{choice}", selected: choice == mode, "{choice}" }
                }
            }
        } else {
            div { class: "combo dial idle", {lens::mode_label(Mode::Base)} }
        }
    }
}

#[component]
pub(crate) fn BlockToggle(versus: Signal<bool>, store: &'static str) -> Element {
    let platform = use_context::<Platform>();
    let on = versus();

    rsx! {
        button {
            class: if on { "btn btn-toggle active versus-btn" } else { "btn btn-toggle versus-btn" },
            onclick: move |_| {
                let next = !*versus.peek();

                versus.set(next);
                app::remember_block(&platform, store, next);
            },
            {lens::block_label(on)}
        }
    }
}
