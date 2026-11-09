use std::sync::atomic::{AtomicU32, Ordering};

use dioxus::prelude::*;

use crate::platform::Platform;
use crate::systems::turn;

use super::drawer;

const KNOB: f64 = 16.0;

static SERIAL: AtomicU32 = AtomicU32::new(0);

#[component]
pub(crate) fn Slider(#[props(default)] class: String, min: i32, max: i32, #[props(default = 1)] step: i32, value: i32, oninput: EventHandler<i32>) -> Element {
    let platform = use_context::<Platform>();
    let drawers = drawer::drawers();
    let turn = turn::turn();
    let serial = use_hook(|| SERIAL.fetch_add(1, Ordering::Relaxed));
    let mut track: Signal<Option<(f64, f64)>> = use_signal(|| None);
    let mut mount: Signal<Option<MountedEvent>> = use_signal(|| None);
    let span = (max - min).max(0);
    let stride = step.max(1);
    let pick = move |at: f64, (left, width): (f64, f64)| -> i32 {
        let reach = (width - KNOB).max(1.0);
        let fraction = ((at - left - KNOB / 2.0) / reach).clamp(0.0, 1.0);
        let steps = (fraction * f64::from(span) / f64::from(stride)).round() as i32;

        (min + steps * stride).clamp(min, max.max(min))
    };

    if !turn.turned() {
        return rsx! {
            input {
                class: "slider {class}",
                r#type: "range",
                min: "{min}",
                max: "{max}",
                step: "{stride}",
                value: "{value}",
                onpointerdown: move |event| drawers.claim(event.data().pointer_id()),
                oninput: move |event| {
                    if let Ok(next) = event.value().parse::<i32>() {
                        oninput.call(next);
                    }
                },
            }
        };
    }

    let fraction = if span > 0 { f64::from(value - min) / f64::from(span) } else { 0.0 };
    let percent = fraction * 100.0;
    let inset = KNOB * fraction;
    let id = format!("slider-{serial}");
    let held = id.clone();

    rsx! {
        div {
            id: "{id}",
            class: "slider slider-turned {class}",
            onmounted: move |event| mount.set(Some(event)),
            onpointerdown: move |event| {
                let pointer = event.data().pointer_id();
                let point = event.data().client_coordinates();
                let at = turn.point(point.x, point.y).0;

                drawers.claim(pointer);
                platform.host().capture_pointer(&held, pointer);

                if let Some(mounted) = mount.peek().clone() {
                    spawn(async move {
                        if let Ok(rect) = mounted.get_client_rect().await {
                            let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);
                            let next = pick(at, (area.x, area.width));

                            track.set(Some((area.x, area.width)));

                            if next != value {
                                oninput.call(next);
                            }
                        }
                    });
                }
            },
            onpointermove: move |event| {
                let Some(rail) = *track.peek() else {
                    return;
                };
                let point = event.data().client_coordinates();
                let next = pick(turn.point(point.x, point.y).0, rail);

                if next != value {
                    oninput.call(next);
                }
            },
            onpointerup: move |_| track.set(None),
            onpointercancel: move |_| track.set(None),
            div { class: "slider-track", div { class: "slider-fill", style: "width: {percent}%;" } }
            div { class: "slider-knob", style: "left: calc({percent}% - {inset}px);" }
        }
    }
}
