use dioxus::prelude::*;

use crate::app::Route;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TERMS_HEAD: &str = "Non-commercial";
const TERMS_TAIL: &str = "use only";
const DISCLAIMER_HEAD: &str = "Not affiliated with";
const DISCLAIMER_TAIL: &str = "PONOS Corp.";
const VERSION_HEAD: &str = "Version";

#[derive(Clone, Copy, PartialEq, Default)]
struct Form {
    terms: f64,
    disclaimer: f64,
    version: f64,
}

impl Form {
    fn measured(self) -> bool {
        self.terms > 0.0 && self.disclaimer > 0.0 && self.version > 0.0
    }

    fn fit(self, row: f64) -> f64 {
        (row - 2.0 * self.terms.max(self.version)) / self.disclaimer
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
struct Footing {
    row: f64,
    single: Form,
    double: Form,
}

struct Placed {
    ready: bool,
    double: bool,
    scale: f64,
}

impl Footing {
    fn place(self) -> Placed {
        if self.row <= 0.0 || !self.single.measured() || !self.double.measured() {
            return Placed { ready: false, double: false, scale: 1.0 };
        }

        if self.single.fit(self.row) >= 1.0 {
            return Placed { ready: true, double: false, scale: 1.0 };
        }

        Placed { ready: true, double: true, scale: self.double.fit(self.row).clamp(0.0, 1.0) }
    }
}

#[component]
fn Probe(class: &'static str, onwidth: EventHandler<f64>, children: Element) -> Element {
    rsx! {
        div {
            class: "{class} home-probe",
            onresize: move |event| {
                if let Ok(size) = event.data().get_border_box_size() {
                    onwidth.call(size.width);
                }
            },
            {children}
        }
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
                style: "visibility: {shown};",
                onresize: move |event| {
                    if let Ok(size) = event.data().get_content_box_size() {
                        footing.with_mut(|held| held.row = size.width);
                    }
                },
                Probe { class: "home-terms", onwidth: move |width| footing.with_mut(|held| held.single.terms = width), "{TERMS_HEAD} {TERMS_TAIL}" }
                Probe { class: "home-disclaimer", onwidth: move |width| footing.with_mut(|held| held.single.disclaimer = width), "{DISCLAIMER_HEAD} {DISCLAIMER_TAIL}" }
                Probe { class: "home-version", onwidth: move |width| footing.with_mut(|held| held.single.version = width), "v{VERSION}" }
                Probe { class: "home-terms", onwidth: move |width| footing.with_mut(|held| held.double.terms = width),
                    "{TERMS_HEAD}"
                    br {}
                    "{TERMS_TAIL}"
                }
                Probe { class: "home-disclaimer", onwidth: move |width| footing.with_mut(|held| held.double.disclaimer = width),
                    "{DISCLAIMER_HEAD}"
                    br {}
                    "{DISCLAIMER_TAIL}"
                }
                Probe { class: "home-version", onwidth: move |width| footing.with_mut(|held| held.double.version = width),
                    "{VERSION_HEAD}"
                    br {}
                    "{VERSION}"
                }
                if placed.double {
                    div { class: "home-terms weak",
                        "{TERMS_HEAD}"
                        br {}
                        "{TERMS_TAIL}"
                    }
                    div { class: "home-disclaimer weak", style: "font-size: {placed.scale}em;",
                        "{DISCLAIMER_HEAD}"
                        br {}
                        "{DISCLAIMER_TAIL}"
                    }
                    div { class: "home-version weak",
                        "{VERSION_HEAD}"
                        br {}
                        "{VERSION}"
                    }
                } else {
                    div { class: "home-terms weak", "{TERMS_HEAD} {TERMS_TAIL}" }
                    div { class: "home-disclaimer weak", "{DISCLAIMER_HEAD} {DISCLAIMER_TAIL}" }
                    div { class: "home-version weak", "v{VERSION}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Footing, Form};

    const SINGLE: Form = Form { terms: 160.0, disclaimer: 190.0, version: 68.0 };
    const DOUBLE: Form = Form { terms: 100.0, disclaimer: 112.0, version: 62.0 };

    fn placed(row: f64) -> (bool, f64) {
        let placed = Footing { row, single: SINGLE, double: DOUBLE }.place();

        assert!(placed.ready);

        (placed.double, placed.scale)
    }

    #[test]
    fn one_line_holds_exactly_as_long_as_it_fits() {
        // 190 centered leaves 160 a side at 510
        assert_eq!(placed(510.0), (false, 1.0));
        assert_eq!(placed(509.0), (true, 1.0));
    }

    #[test]
    fn two_lines_shrink_the_disclaimer_only_to_what_the_corners_leave() {
        assert_eq!(placed(312.0), (true, 1.0));
        assert_eq!(placed(256.0), (true, 0.5));
    }

    #[test]
    fn nothing_shows_until_every_width_is_in() {
        assert!(!Footing { row: 400.0, single: SINGLE, double: Form::default() }.place().ready);
    }
}
