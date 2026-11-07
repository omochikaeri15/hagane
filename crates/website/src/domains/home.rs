use dioxus::prelude::*;
use tracing::warn;

use crate::app::Route;
use crate::platform::Platform;
use crate::systems::turn;

const REPO: &str = "https://github.com/omochikaeri15/hagane";
const SITE: &str = "https://hagane.app";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const COPIED: &str = "Link Copied!";
const DISCLAIMER_HEAD: &str = "Not affiliated with";
const DISCLAIMER_TAIL: &str = "PONOS Corp.";
const COPIED_MS: u32 = 2000;
const TIP_GAP: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Default)]
struct Footing {
    row: f64,
    left: f64,
    slot: f64,
    line: f64,
    head: f64,
}

struct Placed {
    ready: bool,
    split: bool,
    scale: f64,
    shift: f64,
}

impl Footing {
    fn place(self) -> Placed {
        if self.row <= 0.0 || self.slot <= 0.0 || self.line <= 0.0 || self.head <= 0.0 {
            return Placed { ready: false, split: false, scale: 1.0, shift: 0.0 };
        }

        let split = self.slot < self.line;
        let widest = if split { self.head } else { self.line };
        let scale = (self.slot / widest).min(1.0);
        let width = widest * scale;
        let shift = (self.row / 2.0 - self.left - width / 2.0).clamp(0.0, (self.slot - width).max(0.0));

        Placed { ready: true, split, scale, shift }
    }
}

#[component]
pub(crate) fn Home() -> Element {
    let platform = use_context::<Platform>();
    let mut anchor: Signal<Option<MountedEvent>> = use_signal(|| None);
    let mut copied: Signal<Option<(f64, f64)>> = use_signal(|| None);
    let mut round = use_signal(|| 0u32);
    let mut footing = use_signal(Footing::default);
    let placed = footing().place();
    let shown = if placed.ready { "visible" } else { "hidden" };
    let turn = turn::turn();
    let share = move |_| {
        let platform = platform.clone();

        spawn(async move {
            if let Err(fault) = platform.host().copy_text(SITE).await {
                warn!("The site link could not be copied: {fault}");

                return;
            }

            let Some(anchor) = anchor() else {
                return;
            };
            let Ok(rect) = anchor.get_client_rect().await else {
                return;
            };
            let next = *round.peek() + 1;

            let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);

            round.set(next);
            copied.set(Some((area.x + area.width / 2.0, area.y - TIP_GAP)));
            platform.host().sleep(COPIED_MS).await;

            if *round.peek() == next {
                copied.set(None);
            }
        });
    };

    rsx! {
        div { class: "home",
            div { class: "home-main",
                div { class: "home-title", "Hagane" }
                div { class: "home-subtitle weak", "Cat Game Suite" }
                div { class: "home-group", "Game" }
                div { class: "home-row",
                    Link { to: Route::Sandbox {}, class: "btn btn-primary home-nav", "Sandbox" }
                    Link { to: Route::Cats { segments: Vec::new() }, class: "btn btn-primary home-nav", "Cats" }
                    Link { to: Route::Enemies { segments: Vec::new() }, class: "btn btn-primary home-nav", "Enemies" }
                    Link { to: Route::Stages { segments: Vec::new() }, class: "btn btn-primary home-nav", "Stages" }
                }
                div { class: "home-group", "Data" }
                div { class: "home-row",
                    Link { to: Route::Mods {}, class: "btn btn-primary home-nav", "Mods" }
                }
                div { class: "home-group", "Other" }
                div { class: "home-row",
                    Link { to: Route::Settings {}, class: "btn btn-primary home-nav", "Settings" }
                }
            }
            div {
                class: "home-footer",
                onresize: move |event| {
                    if let Ok(size) = event.data().get_content_box_size() {
                        footing.with_mut(|held| held.row = size.width);
                    }
                },
                div {
                    class: "home-links home-links-left",
                    onresize: move |event| {
                        if let Ok(size) = event.data().get_border_box_size() {
                            footing.with_mut(|held| held.left = size.width);
                        }
                    },
                    a { class: "btn btn-text", href: "{REPO}/releases/tag/v{VERSION}", target: "_blank", rel: "noopener", "v{VERSION}" }
                    span { class: "home-sep", "|" }
                    a { class: "btn btn-text", href: "{REPO}/releases", target: "_blank", rel: "noopener", "Changelogs" }
                }
                div {
                    class: "home-disclaimer weak",
                    onresize: move |event| {
                        if let Ok(size) = event.data().get_content_box_size() {
                            footing.with_mut(|held| held.slot = size.width);
                        }
                    },
                    span {
                        class: "home-disclaimer-probe",
                        onresize: move |event| {
                            if let Ok(size) = event.data().get_border_box_size() {
                                footing.with_mut(|held| held.line = size.width);
                            }
                        },
                        "{DISCLAIMER_HEAD} {DISCLAIMER_TAIL}"
                    }
                    span {
                        class: "home-disclaimer-probe",
                        onresize: move |event| {
                            if let Ok(size) = event.data().get_border_box_size() {
                                footing.with_mut(|held| held.head = size.width);
                            }
                        },
                        "{DISCLAIMER_HEAD}"
                    }
                    span { class: "home-disclaimer-text", style: "font-size: {placed.scale}em; left: {placed.shift}px; visibility: {shown};",
                        "{DISCLAIMER_HEAD}"
                        if placed.split {
                            br {}
                        } else {
                            " "
                        }
                        "{DISCLAIMER_TAIL}"
                    }
                }
                div { class: "home-links home-links-right",
                    button { class: "btn btn-text", onmounted: move |event| anchor.set(Some(event)), onclick: share, "Share" }
                    if let Some((x, y)) = copied() {
                        span { class: "tip-body bordered-box", style: "left: {x}px; top: {y}px;", "{COPIED}" }
                    }
                    span { class: "home-sep", "|" }
                    a { class: "btn btn-text", href: REPO, target: "_blank", rel: "noopener", "GitHub" }
                }
            }
        }
    }
}
