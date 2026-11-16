use dioxus::prelude::*;

use crate::app::Route;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TERMS: &str = "Non-commercial use only";
const DISCLAIMER_HEAD: &str = "Not affiliated with";
const DISCLAIMER_TAIL: &str = "PONOS Corp.";

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
    let mut footing = use_signal(Footing::default);
    let placed = footing().place();
    let shown = if placed.ready { "visible" } else { "hidden" };

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
                    class: "home-terms weak",
                    onresize: move |event| {
                        if let Ok(size) = event.data().get_border_box_size() {
                            footing.with_mut(|held| held.left = size.width);
                        }
                    },
                    "{TERMS}"
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
                div { class: "home-version weak", "v{VERSION}" }
            }
        }
    }
}
