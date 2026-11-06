use dioxus::prelude::*;
use dioxus::router::Navigator;

use codex::chapter::Category;
use foundry::domains::settings::Settings as GameSettings;
use foundry::domains::stage::GlobalStageId;

use crate::domains::cat::{self, Cats};
use crate::domains::enemy::{self, Enemies};
use crate::domains::home::Home;
use crate::domains::mods::{self, Mods};
use crate::domains::sandbox::{self, Sandbox};
use crate::domains::settings::{self, Settings};
use crate::domains::stage::{self, Stages};
use crate::platform::Platform;
use crate::systems::library::Library;
use crate::systems::{animation, catalog, context, export, images, turn};
use crate::theme::{self, AppTheme};
use crate::widget::{drawer, tip};

static ICON: Asset = asset!("/assets/icon.ico");
pub(crate) const THEME_KEY: &str = "theme";
const SETTINGS_KEY: &str = "settings";
const SEPARATOR: char = '/';
const FORM_LETTERS: [&str; 4] = ["f", "c", "s", "u"];
const OPEN_ARROW: &str = "\u{25b6}";
const CLOSED_ARROW: &str = "\u{25c0}";
const LAYOUT_PROBE: f64 = 1.5;

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/")]
        Home {},
        #[route("/cats/:..segments")]
        Cats { segments: Vec<String> },
        #[route("/enemies/:..segments")]
        Enemies { segments: Vec<String> },
        #[route("/stages/:..segments")]
        Stages { segments: Vec<String> },
        #[route("/mods")]
        Mods {},
        #[route("/sandbox")]
        Sandbox {},
        #[route("/settings")]
        Settings {},
}

const TABS: [Route; 7] = [
    Route::Home {},
    Route::Sandbox {},
    Route::Cats { segments: Vec::new() },
    Route::Enemies { segments: Vec::new() },
    Route::Stages { segments: Vec::new() },
    Route::Mods {},
    Route::Settings {},
];

impl Route {
    pub fn cat(id: u32) -> Self {
        Self::Cats { segments: vec![format!("{id:03}")] }
    }

    pub fn enemy(id: u32) -> Self {
        Self::Enemies { segments: vec![format!("{id:03}")] }
    }

    pub(crate) fn cat_form(id: u32, form: usize) -> Self {
        let mut segments = vec![format!("{id:03}")];

        segments.extend(FORM_LETTERS.get(form).map(|letter| (*letter).to_owned()));

        Self::Cats { segments }
    }

    pub(crate) fn stage(id: &GlobalStageId) -> Option<Self> {
        let prefix = id.category.map_prefix();

        (!prefix.is_empty()).then(|| Self::Stages { segments: vec![prefix, format!("{:03}", id.map), format!("{:03}", id.stage)] })
    }

    pub fn picked(segments: &[String]) -> Option<u32> {
        segments.first()?.parse().ok()
    }

    pub(crate) fn form_of(segments: &[String]) -> Option<usize> {
        let letter = segments.get(1)?;

        FORM_LETTERS.iter().position(|known| known.eq_ignore_ascii_case(letter))
    }

    pub(crate) fn stage_of(segments: &[String]) -> Option<GlobalStageId> {
        let [prefix, map, stage] = segments else {
            return None;
        };

        Some(GlobalStageId { category: Category::from_prefix(prefix), map: map.parse().ok()?, stage: stage.parse().ok()? })
    }

    fn segments(&self) -> &[String] {
        match self {
            Self::Cats { segments } | Self::Enemies { segments } | Self::Stages { segments } => segments,
            _ => &[],
        }
    }

    fn joined(segments: &[String]) -> String {
        segments.join(&SEPARATOR.to_string())
    }

    fn split(saved: &str) -> Vec<String> {
        saved.split(SEPARATOR).filter(|segment| !segment.is_empty()).map(str::to_owned).collect()
    }

    pub fn tab_name(&self) -> &'static str {
        match self {
            Self::Home {} => "Home",
            Self::Cats { .. } => "Cats",
            Self::Enemies { .. } => "Enemies",
            Self::Stages { .. } => "Stages",
            Self::Mods {} => "Mods",
            Self::Sandbox {} => "Sandbox",
            Self::Settings {} => "Settings",
        }
    }
}

pub fn remember_settings(platform: &Platform, settings: &GameSettings) {
    if let Ok(json) = serde_json::to_string(settings) {
        platform.host().remember(SETTINGS_KEY, &json);
    }
}

#[component]
pub fn App() -> Element {
    let platform = use_context::<Platform>();
    let saved = platform.host().recall(THEME_KEY).and_then(|key| AppTheme::from_key(&key)).unwrap_or_default();

    use_context_provider(|| Signal::new(saved));

    let restored = platform.host().recall(SETTINGS_KEY).and_then(|json| serde_json::from_str::<GameSettings>(&json).ok()).unwrap_or_default();
    let settings = use_context_provider(|| Signal::new(restored));
    let pictures = images::provide(&platform);
    animation::provide_rigs(&platform);

    sandbox::provide(&platform);
    settings::provide_logging(&platform);
    settings::provide_fullscreen(&platform);
    drawer::provide();
    tip::provide();

    let turn = turn::provide(&platform);
    let angler = platform.clone();
    let library = Library::provide(platform);

    use_hook(move || {
        pictures.attach(move || {
            let payload = library.payload()?;

            (!library.modded()).then(|| images::epoch_of(&payload))
        });
    });

    let globals = context::provide(library, settings);
    let mut fonts_due = use_signal(|| false);

    use_effect(move || {
        if globals().is_some() && !*fonts_due.peek() {
            fonts_due.set(true);
        }
    });
    catalog::provide();
    export::provide();
    cat::provide();
    animation::provide();
    enemy::provide();
    stage::provide();
    mods::provide();

    rsx! {
        document::Link { rel: "icon", href: ICON }
        if fonts_due() {
            for (family, face, _) in theme::FONT_FACES {
                link { key: "{family}", rel: "prefetch", r#as: "font", r#type: "font/ttf", href: "{face}", crossorigin: "anonymous", "fetchpriority": "low" }
            }
        }
        document::Style { {theme::stylesheet()} }
        document::Link { rel: "stylesheet", href: theme::THEME }
        div {
            class: "viewport-probe",
            onresize: move |event| {
                if let Ok(size) = event.data().get_border_box_size() {
                    turn.measure(size.width, size.height, angler.host().screen_angle());
                }
            },
        }
        Router::<Route> {}
    }
}

pub(crate) fn use_last_visited(key: &'static str, segments: &[String], make: fn(Vec<String>) -> Route) {
    let platform = use_context::<Platform>();
    let navigator = use_navigator();
    let bare = segments.is_empty();

    use_effect(use_reactive!(|bare| {
        if !bare {
            return;
        }

        if let Some(saved) = last_visited(&platform, key) {
            navigator.replace(make(saved));
        }
    }));
}

pub(crate) fn last_visited(platform: &Platform, key: &str) -> Option<Vec<String>> {
    platform.host().recall(key).map(|saved| Route::split(&saved)).filter(|saved| !saved.is_empty())
}

pub(crate) fn remember_visit(platform: &Platform, key: &str, route: &Route) {
    platform.host().remember(key, &Route::joined(route.segments()));
}

pub(crate) fn forget_visit(platform: &Platform, navigator: Navigator, key: &str, bare: Route) {
    platform.host().remember(key, "");
    navigator.replace(bare);
}

#[component]
fn Shell() -> Element {
    let platform = use_context::<Platform>();
    let theme = use_context::<Signal<AppTheme>>();
    let drawers = drawer::drawers();
    let tips = tip::tips();
    let turn = turn::turn();
    let mut nav = drawers.nav;
    let mut compact = drawers.compact;
    let mut narrow = drawers.narrow;
    let route = use_route::<Route>();
    let (turned, frame) = turn.frame();
    let class = turned.map_or_else(|| format!("app {}", theme().class()), |turned| format!("app {turned} {}", theme().class()));

    use_effect(move || {
        if compact() {
            nav.set(false);
        }
    });

    let presser = platform.clone();
    let follower = platform;

    rsx! {
        div {
            class: "{class}",
            style: "{frame}",
            onpointerdown: move |event| {
                let data = event.data();
                let point = data.client_coordinates();

                tips.dismiss();
                drawers.press(data.pointer_id(), &data.pointer_type(), turn.point(point.x, point.y), presser.host().now());
            },
            onpointermove: move |event| {
                let data = event.data();
                let point = data.client_coordinates();

                drawers.follow(data.pointer_id(), turn.point(point.x, point.y), follower.host().now());
            },
            onpointerup: move |event| drawers.release(event.data().pointer_id()),
            onpointercancel: move |event| drawers.abandon(event.data().pointer_id()),
            div {
                class: "layout-probe",
                onresize: move |event| {
                    if let Ok(size) = event.data().get_border_box_size() {
                        let (touching, slim) = (size.width > LAYOUT_PROBE, size.height > LAYOUT_PROBE);

                        if *compact.peek() != touching {
                            compact.set(touching);
                        }

                        if *narrow.peek() != slim {
                            narrow.set(slim);
                        }
                    }
                },
            }
            div { class: "content", Outlet::<Route> {} }
            Nav { route: route.tab_name() }
        }
    }
}

#[component]
fn Nav(route: &'static str) -> Element {
    let drawers = drawer::drawers();
    let mut open = drawers.nav;
    let shown = open();
    let pull = drawers.pulled(drawer::Side::Right);
    let (slot, style) = drawer::styled(if shown { "sidebar-slot" } else { "sidebar-slot closed" }, false, pull);
    let arrow = if shown { OPEN_ARROW } else { CLOSED_ARROW };

    rsx! {
        if shown {
            div { class: "drawer-scrim nav-scrim", onclick: move |_| open.set(false) }
        }
        div { class: "sidebar-layer",
            button { class: "btn btn-primary nav-toggle symbols", onclick: move |_| open.toggle(), "{arrow}" }
            div { class: "{slot}", style: "{style}",
                nav { class: "sidebar",
                    div { class: "sidebar-scroll",
                        div { class: "tabs",
                            for tab in TABS {
                                Link {
                                    to: tab.clone(),
                                    class: if route == tab.tab_name() { "tab active" } else { "tab" },
                                    onclick: move |_| {
                                        if *drawers.compact.peek() {
                                            open.set(false);
                                        }
                                    },
                                    "{tab.tab_name()}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
