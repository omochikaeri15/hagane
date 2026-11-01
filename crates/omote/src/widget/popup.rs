use dioxus::prelude::*;

use super::drawer;

#[component]
pub(crate) fn Popup(title: String, open: Signal<bool>, #[props(default = 600.0)] width: f32, #[props(default)] modal: bool, #[props(default)] class: String, children: Element) -> Element {
    let drawers = drawer::drawers();

    if !open() {
        return rsx! {};
    }

    let backdrop = if modal { "scrim modal" } else { "scrim" };

    rsx! {
        div { class: "{backdrop}", onclick: move |_| open.set(false), onpointerdown: move |event| drawers.claim(event.data().pointer_id()),
            div {
                class: "popup {class}",
                style: "max-width: {width}px;",
                onclick: move |event| event.stop_propagation(),
                div { class: "popup-header",
                    span { class: "popup-title", "{title}" }
                    button { class: "popup-close symbols", onclick: move |_| open.set(false), "\u{2715}" }
                }
                div { class: "popup-body", {children} }
            }
        }
    }
}
