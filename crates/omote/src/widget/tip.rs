use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use crate::systems::turn;

use super::Fitted;

const GAP: f64 = 4.0;
const EDGE: f64 = 8.0;
const MOUSE: &str = "mouse";

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
pub(crate) struct Tips(Signal<Option<u64>>);

pub(crate) fn provide() -> Tips {
    use_context_provider(|| Tips(Signal::new(None)))
}

pub(crate) fn tips() -> Tips {
    use_context::<Tips>()
}

impl Tips {
    pub(crate) fn dismiss(self) {
        let mut open = self.0;

        if open.peek().is_some() {
            open.set(None);
        }
    }

    fn open(self, id: u64) {
        let mut open = self.0;

        open.set(Some(id));
    }

    fn close(self, id: u64) {
        let mut open = self.0;

        if *open.peek() == Some(id) {
            open.set(None);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Spot {
    x: f64,
    top: f64,
    bottom: f64,
    shift: f64,
    below: bool,
}

#[component]
pub(crate) fn Tip(text: String, #[props(default)] touch: bool, #[props(default)] hover: bool, children: Element) -> Element {
    let tips = tips();
    let turn = turn::turn();
    let fitted = try_use_context::<Fitted>();
    let id = use_hook(|| NEXT.fetch_add(1, Ordering::Relaxed));
    let shown = use_memo(move || (tips.0)() == Some(id));
    let mut anchor: Signal<Option<MountedEvent>> = use_signal(|| None);
    let mut spot: Signal<Option<Spot>> = use_signal(|| None);
    let mut mouse = use_signal(|| true);
    let mut hovering = use_signal(|| false);
    let show = move |hover: bool| {
        let Some(anchor) = anchor() else {
            return;
        };

        spawn(async move {
            let Ok(rect) = anchor.get_client_rect().await else {
                return;
            };

            if hover && !*hovering.peek() {
                return;
            }

            let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);

            spot.set(Some(Spot { x: area.x + area.width / 2.0, top: area.y - GAP, bottom: area.y + area.height + GAP, shift: 0.0, below: false }));
            tips.open(id);
        });
    };
    let fit = move |event: MountedEvent| {
        spawn(async move {
            let (Ok(rect), Some(held)) = (event.get_client_rect().await, *spot.peek()) else {
                return;
            };
            let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);
            let (width, _) = turn.extent();
            let past = area.x + area.width - (width - EDGE);
            let shift = if area.x < EDGE {
                EDGE - area.x
            } else if past > 0.0 {
                -past.min(area.x - EDGE)
            } else {
                0.0
            };

            spot.set(Some(Spot { shift, below: area.y < EDGE, ..held }));
        });
    };
    let open = shown();
    let placed = spot().filter(|_| open).map(|at| (at.x + at.shift, if at.below { at.bottom } else { at.top }, at.below));
    let unzoom = fitted.map_or(1.0, |fitted| 1.0 / (fitted.0)().max(f32::EPSILON));

    rsx! {
        span {
            class: "tip",
            onmounted: move |event| anchor.set(Some(event)),
            onpointerdown: move |event| mouse.set(event.data().pointer_type() == MOUSE),
            onpointerenter: move |event| {
                if !touch && event.data().pointer_type() == MOUSE {
                    hovering.set(true);
                    show(true);
                }
            },
            onpointerleave: move |event| {
                if event.data().pointer_type() == MOUSE {
                    hovering.set(false);
                    tips.close(id);
                }
            },
            onclick: move |_| {
                let pointed = mouse();

                if (!touch || !pointed) && (!hover || pointed) {
                    show(false);
                }
            },
            {children}
        }
        if let Some((left, top, below)) = placed {
            span {
                class: if below { "tip-body below bordered-box" } else { "tip-body bordered-box" },
                style: "left: {left}px; top: {top}px; zoom: {unzoom};",
                onmounted: fit,
                "{text}"
            }
        }
    }
}
