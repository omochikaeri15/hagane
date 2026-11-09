use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

use crate::systems::turn;

use super::drawer;

const KEPT_ACROSS: f64 = 50.0;
const KEPT_DOWN: f64 = 30.0;

#[derive(Clone, Copy, PartialEq)]
struct Grip {
    from: (f64, f64),
    offset: (f64, f64),
    low: (f64, f64),
    high: (f64, f64),
}

impl Grip {
    fn carried(&self, at: (f64, f64)) -> (f64, f64) {
        let across = self.offset.0 + at.0 - self.from.0;
        let down = self.offset.1 + at.1 - self.from.1;

        (across.max(self.low.0).min(self.high.0), down.max(self.low.1).min(self.high.1))
    }
}

#[component]
pub(crate) fn Popup(title: String, open: Signal<bool>, #[props(default = 600.0)] width: f32, #[props(default)] modal: bool, #[props(default)] class: String, children: Element) -> Element {
    let drawers = drawer::drawers();
    let turn = turn::turn();
    let mut offset = use_signal(|| (0.0f64, 0.0f64));
    let mut grip: Signal<Option<Grip>> = use_signal(|| None);
    let mut carried = use_signal(|| false);
    let mut frame: Signal<Option<MountedEvent>> = use_signal(|| None);

    if !open() {
        return rsx! {};
    }

    let backdrop = if modal { "scrim modal" } else { "scrim" };
    let (across, down) = offset();

    rsx! {
        div {
            class: "{backdrop}",
            onclick: move |_| {
                if *carried.peek() {
                    carried.set(false);

                    return;
                }

                open.set(false);
            },
            onpointerdown: move |event| drawers.claim(event.data().pointer_id()),
            onpointermove: move |event| {
                let Some(held) = *grip.peek() else {
                    return;
                };

                if event.data().held_buttons().is_empty() {
                    grip.set(None);

                    return;
                }

                let point = event.data().client_coordinates();
                let next = held.carried(turn.point(point.x, point.y));

                if next != *offset.peek() {
                    carried.set(true);
                    offset.set(next);
                }
            },
            onpointerup: move |_| grip.set(None),
            onpointercancel: move |_| grip.set(None),
            div {
                class: "popup {class}",
                style: "max-width: {width}px; left: {across}px; top: {down}px;",
                onmounted: move |event| frame.set(Some(event)),
                onclick: move |event| {
                    carried.set(false);
                    event.stop_propagation();
                },
                div {
                    class: "popup-header",
                    onpointerdown: move |event| {
                        if event.data().trigger_button() != Some(MouseButton::Primary) {
                            return;
                        }

                        let point = event.data().client_coordinates();
                        let held = *offset.peek();

                        carried.set(false);
                        grip.set(Some(Grip { from: turn.point(point.x, point.y), offset: held, low: (f64::NEG_INFINITY, f64::NEG_INFINITY), high: (f64::INFINITY, f64::INFINITY) }));

                        if let Some(mounted) = frame.peek().clone() {
                            spawn(async move {
                                let Ok(rect) = mounted.get_client_rect().await else {
                                    return;
                                };
                                let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);
                                let (wide, tall) = turn.extent();
                                let (left, top) = (area.x - held.0, area.y - held.1);

                                if wide <= 0.0 || tall <= 0.0 {
                                    return;
                                }

                                if let Some(gripped) = grip.write().as_mut() {
                                    gripped.low = (KEPT_ACROSS - area.width - left, -top);
                                    gripped.high = (wide - KEPT_ACROSS - left, tall - KEPT_DOWN - top);
                                }
                            });
                        }
                    },
                    span { class: "popup-title", "{title}" }
                    button { class: "popup-close symbols", onpointerdown: move |event| {
                        event.stop_propagation();
                        drawers.claim(event.data().pointer_id());
                    }, onclick: move |_| open.set(false), "\u{2715}" }
                }
                div { class: "popup-body", {children} }
            }
        }
    }
}
