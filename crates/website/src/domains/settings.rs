use std::collections::HashSet;
use std::sync::Arc;

use dioxus::prelude::*;
use tracing::warn;

use codex::chapter::treasure::ARC_SLOTS;
use foundry::domains::cat::files as cat_files;
use foundry::domains::sandbox::keybind::Bind;
use foundry::domains::settings::Settings as GameSettings;
use foundry::domains::settings::{AnimSettings, COMBAT_HEIGHT_FLOOR, CatDataSettings, COMBAT_OPACITY_CEILING, CombatLayer, HitPositions, ReplaySource, SidebarBehavior, lang};
use foundry::systems::treasure::{ARCS, Catalog, Config as TreasureConfig, FULL_PERCENT};

use crate::app::{Route, remember_settings};
use crate::platform::{FILE_CHECK_DEFAULT, FILE_CHECK_KEY, LOGGING_KEY, LOGGING_ON, Platform, URL_DOMAIN, URL_FOLLOWING, URL_TYPE_KEY};
use crate::systems::animation::{self, Rigs};
use crate::systems::catalog;
use crate::systems::context::{self, Shared};
use crate::systems::library::Library;
use crate::systems::images;
use crate::systems::turn::{self, Orientation};
use crate::widget::{Drawer, Tip, Toggle, drawer};

const CHAPTER_TITLES: [[&str; 3]; 3] = [
    ["Empire of Cats Ch. 1", "Empire of Cats Ch. 2", "Empire of Cats Ch. 3"],
    ["Into the Future Ch. 1", "Into the Future Ch. 2", "Into the Future Ch. 3"],
    ["Cats of the Cosmos Ch. 1", "Cats of the Cosmos Ch. 2", "Cats of the Cosmos Ch. 3"],
];
const ENTRY_DIGITS: usize = 4;
const NONE_CODE: &str = "--";
const LANGUAGE_ROW_HEIGHT: f64 = 32.0;

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Idle,
    Pressed { index: usize, last: f64 },
    Moving { index: usize, last: f64, carry: f64 },
}

fn dragging_now(drag: Signal<Drag>) -> bool {
    *drag.peek() != Drag::Idle
}

fn dragged(drag: Drag) -> Option<usize> {
    match drag {
        Drag::Idle => None,
        Drag::Pressed { index, .. } | Drag::Moving { index, .. } => Some(index),
    }
}

fn carry(order: &mut [String], index: usize, carry: f64) -> (usize, f64) {
    let last = order.len().saturating_sub(1);
    let (mut index, mut carry) = (index, carry);

    while carry >= LANGUAGE_ROW_HEIGHT && index < last {
        order.swap(index, index + 1);
        index += 1;
        carry -= LANGUAGE_ROW_HEIGHT;
    }

    while carry <= -LANGUAGE_ROW_HEIGHT && index > 0 {
        order.swap(index, index - 1);
        index -= 1;
        carry += LANGUAGE_ROW_HEIGHT;
    }

    if (index == last && carry > 0.0) || (index == 0 && carry < 0.0) {
        carry = 0.0;
    }

    (index, carry)
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    Cats,
    Enemies,
    Stages,
    Sandbox,
    Animation,
    About,
}

const TABS: [(Tab, &str); 7] = [
    (Tab::General, "General"),
    (Tab::Cats, "Cats"),
    (Tab::Enemies, "Enemies"),
    (Tab::Stages, "Stages"),
    (Tab::Sandbox, "Sandbox"),
    (Tab::Animation, "Animation"),
    (Tab::About, "About"),
];
const HIT_FADE_HINT: &str = "How many frames a hit box stays lit after its hit lands";
const SYSTEM_HEIGHT_HINT: &str = "How tall the boxes under the unit may grow, 3 at the least";
const SYSTEM_OFFSET_HINT: &str = "Moves the boxes up from the ground by this much, or down when negative";
const FILL_OPACITY_HINT: &str = "How solid a filled box is, from 0 to 100";
const FULLSCREEN_HINT: &str = "Fills the whole screen with the app\nF11 switches this at any time";
const FILE_CHECK_HINT: &str = "How often, in seconds, the site looks for new game files on the file host\nLeave it empty to never look";
const FILE_CHECK_OFF: &str = "Off";
const FILE_CHECK_DIGITS: usize = 5;
const ORIENTATION_HINT: &str = "Automatic follows the browser\nPortrait keeps the app upright against the device's own top\nLandscape turns it sideways while the screen stands upright, so hold the device with its right side up";
const LOGGING_HINT: &str = "Enables logs for easy debugging\nDisable to improve performance\nDevs may refuse to debug without logs";
const LOGGING_OFF: &str = "off";
const URL_TYPES: [(&str, &str); 2] = [(URL_DOMAIN, "Domain"), (URL_FOLLOWING, "Following")];
const RELOAD_REQUIRED: &str = "!! Reload Required !!";
const LOGGING_DETAIL: &str = "The Logging change applies the next time the page loads";

#[derive(Clone, Copy)]
pub(crate) struct Logging {
    launched: bool,
    chosen: Signal<bool>,
}

#[derive(Clone, Copy)]
pub(crate) struct Fullscreen(Signal<bool>);

pub(crate) fn provide_fullscreen(platform: &Platform) -> Fullscreen {
    let held = use_context_provider(|| Fullscreen(Signal::new(platform.host().fullscreen())));
    let watcher = platform.clone();

    use_hook(move || {
        let shown = held.0;

        watcher.host().fullscreen_watch(Box::new(move |on| {
            let mut shown = shown;

            shown.set(on);
        }));
    });

    held
}

fn file_check_text(platform: &Platform) -> String {
    platform.host().recall(FILE_CHECK_KEY).unwrap_or_else(|| FILE_CHECK_DEFAULT.to_owned())
}

fn file_check_seconds(text: &str) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|seconds| *seconds > 0)
}

pub(crate) fn apply_file_check(platform: &Platform) {
    platform.host().file_check(file_check_seconds(&file_check_text(platform)));
}

pub(crate) fn provide_logging(platform: &Platform) -> Logging {
    let launched = platform.host().recall(LOGGING_KEY).as_deref() == Some(LOGGING_ON);

    use_context_provider(|| Logging { launched, chosen: Signal::new(launched) })
}
const LICENSES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../foundry/assets/licenses.txt"));
const REPLAY_SOURCE_HINT: &str = "Hybrid plays a replay with the game files it was recorded with and your own files for anything it did not keep, BCV uses the recorded files only, VFS uses your own game files only";
const KEY_PROMPT: &str = "Press a key...";
const KEY_UNBOUND: &str = "Unbound";
const SPACE: &str = " ";
const SPACE_NAME: &str = "Space";

fn key_label(name: &str) -> String {
    match name {
        SPACE => SPACE_NAME.to_owned(),
        single if single.chars().count() == 1 => single.to_uppercase(),
        named => named.to_owned(),
    }
}

fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Character(typed) if typed == SPACE => Some(SPACE_NAME.to_owned()),
        Key::Character(typed) => Some(typed.to_lowercase()),
        Key::Unidentified => None,
        named => Some(format!("{named:?}")),
    }
}
const SIDEBAR_BEHAVIORS: [(SidebarBehavior, &str); 2] = [(SidebarBehavior::Cover, "Cover"), (SidebarBehavior::Push, "Push")];

#[derive(Clone)]
pub(crate) struct Treasury(pub(crate) Arc<Catalog>);

impl PartialEq for Treasury {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

fn typable_percent(entry: &str) -> bool {
    entry.len() <= ENTRY_DIGITS && entry.chars().all(|glyph| glyph.is_ascii_digit() || glyph == '%')
}

pub(crate) fn apply(platform: &Platform, mut settings: Signal<GameSettings>, change: impl FnOnce(&mut GameSettings)) {
    change(&mut settings.write());
    remember_settings(platform, &settings.read());
}

#[component]
pub(crate) fn Settings() -> Element {
    let platform = use_context::<Platform>();
    let settings = context::settings();
    let globals = context::globals();
    let mut tab = use_signal(Tab::default);
    let mut tabs_open = use_signal(|| false);
    let current = tab();
    let logging = use_context::<Logging>();
    let reload = (logging.chosen)() != logging.launched;

    rsx! {
        div { class: "settings-page",
            div { class: "toast-slot",
                div { class: if reload { "toast notice-banner" } else { "toast notice-banner hidden" },
                    div { "{RELOAD_REQUIRED}" }
                    div { class: "detail", "{LOGGING_DETAIL}" }
                }
            }
            Drawer { open: tabs_open,
                div { class: "settings-tabs",
                    for (which, label) in TABS {
                        button {
                            class: if current == which { "list-row filled selected settings-tab" } else { "list-row filled settings-tab" },
                            onclick: move |_| {
                                tab.set(which);
                                tabs_open.set(false);
                            },
                            "{label}"
                        }
                    }
                }
            }
            div { class: if current == Tab::About { "settings-body about-body" } else { "settings-body scroll" },
                match current {
                    Tab::General => rsx! { General { platform: platform.clone(), settings, globals } },
                    Tab::Cats => rsx! { Cats { platform: platform.clone(), settings } },
                    Tab::Enemies => rsx! { Enemies { platform: platform.clone(), settings } },
                    Tab::Stages => rsx! { Stages { platform: platform.clone(), settings } },
                    Tab::Sandbox => rsx! { SandboxTab { platform: platform.clone(), settings } },
                    Tab::Animation => rsx! { AnimationTab { platform: platform.clone(), settings } },
                    Tab::About => rsx! { About {} },
                }
            }
        }
    }
}

fn relocalize(platform: &Platform, settings: Signal<GameSettings>, library: Library, catalogs: catalog::Catalogs, pictures: &images::Pictures, rigs: &Rigs, order: Vec<String>) {
    let mut library = library;
    let mut catalogs = catalogs;

    apply(platform, settings, |next| next.general.language_priority = order.clone());
    library.relocalize(&order);
    catalogs.reset();
    pictures.forget_all();
    rigs.forget_all();
}

#[component]
fn About() -> Element {
    rsx! {
        div { class: "about",
            div { class: "about-header",
                div { class: "about-title", "About Hagane" }
                div { "A high-performance cat game toolkit built by Omochi" }
                div { class: "about-subtitle", "Open Source & Legal Info" }
            }
            div { class: "rule" }
            div { class: "about-legal scroll", "{LICENSES}" }
        }
    }
}

#[component]
fn General(platform: Platform, settings: Signal<GameSettings>, globals: Option<Shared>) -> Element {
    let library = use_context::<Library>();
    let catalogs = catalog::catalogs();
    let pictures = images::pictures();
    let rigs = animation::rigs();
    let mut drag = use_signal(|| Drag::Idle);
    let mut working: Signal<Option<Vec<String>>> = use_signal(|| None);
    let fullscreen = (use_context::<Fullscreen>().0)();
    let turn = turn::turn();
    let drawers = drawer::drawers();
    let orientation = turn.mode();
    let mut logged = use_context::<Logging>().chosen;
    let logging = logged();
    let navigator = use_navigator();
    let route = use_route::<Route>();
    let addressed = platform.host().has_address_bar();
    let mut url_type = use_signal(|| platform.host().recall(URL_TYPE_KEY).filter(|kept| kept == URL_FOLLOWING).map_or(URL_DOMAIN, |_| URL_FOLLOWING));
    let url_label = URL_TYPES.iter().find(|(key, _)| *key == url_type()).map_or(URL_TYPES[0].1, |(_, label)| label);
    let mut file_check = use_signal(|| file_check_text(&platform));
    let held = settings.read();
    let saved = held.general.language_priority.clone();
    let priority = working.read().clone().unwrap_or_else(|| saved.clone());
    let none_index = priority.iter().position(|code| code == NONE_CODE);
    let for_reset = platform.clone();
    let pictures_reset = pictures.clone();
    let rigs_reset = rigs.clone();
    let dragging = dragged(drag());
    let release = {
        let platform = platform.clone();
        let pictures = pictures.clone();
        let rigs = rigs.clone();

        move || {
            drag.set(Drag::Idle);

            if let Some(order) = working.take()
                && order != settings.read().general.language_priority
            {
                relocalize(&platform, settings, library, catalogs, &pictures, &rigs, order);
            }
        }
    };
    let mut release_up = release.clone();
    let mut release_cancel = release.clone();
    let mut follow = move |x: f64, y: f64| {
        let (_, y) = turn.point(x, y);

        match drag() {
            Drag::Idle => {}
            Drag::Pressed { index, last: _ } => drag.set(Drag::Moving { index, last: y, carry: 0.0 }),
            Drag::Moving { index, last, carry: held } => {
                let mut order = working.read().clone().unwrap_or_else(|| settings.read().general.language_priority.clone());
                let (index, held) = carry(&mut order, index, held + (y - last));

                working.set(Some(order));
                drag.set(Drag::Moving { index, last: y, carry: held });
            }
        }
    };

    rsx! {
        if dragging.is_some() {
            div {
                class: "drag-scrim",
                onpointermove: move |event| {
                    let point = event.data().client_coordinates();

                    follow(point.x, point.y);
                },
                onpointerup: move |_| release_up(),
                onpointercancel: move |_| release_cancel(),
            }
        }
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "System" }
                if addressed {
                    label { class: "setting-row inline",
                        span { "URL Type" }
                        select { class: "combo", value: "{url_label}",
                            onchange: {
                                let platform = platform.clone();

                                move |event: FormEvent| {
                                    let Some((key, _)) = URL_TYPES.iter().find(|(_, label)| *label == event.value()) else {
                                        return;
                                    };

                                    platform.host().remember(URL_TYPE_KEY, key);
                                    url_type.set(key);
                                    navigator.replace(route.clone());
                                }
                            },
                            for (key, label) in URL_TYPES {
                                option { value: "{label}", selected: key == url_type(), "{label}" }
                            }
                        }
                    }
                }
                div { class: "touch-only",
                    label { class: "setting-row inline", title: ORIENTATION_HINT,
                        span { "Orientation" }
                        select { class: "combo", value: "{orientation}",
                            onchange: {
                                let platform = platform.clone();

                                move |event: FormEvent| {
                                    if let Some(mode) = Orientation::ALL.into_iter().find(|mode| mode.to_string() == event.value()) {
                                        turn.choose(&platform, mode);
                                    }
                                }
                            },
                            for mode in Orientation::ALL {
                                option { value: "{mode}", selected: mode == orientation, "{mode}" }
                            }
                        }
                    }
                }
                label { class: "setting-row inline", title: FILE_CHECK_HINT,
                    span { "File Check Interval" }
                    input { class: "input entry-input", r#type: "text", inputmode: "numeric", placeholder: "{FILE_CHECK_OFF}", value: "{file_check}",
                        oninput: {
                            let platform = platform.clone();

                            move |event: FormEvent| {
                                let entry = event.value();

                                if entry.len() > FILE_CHECK_DIGITS || !entry.chars().all(|glyph| glyph.is_ascii_digit()) {
                                    return;
                                }

                                platform.host().remember(FILE_CHECK_KEY, &entry);
                                platform.host().file_check(file_check_seconds(&entry));
                                file_check.set(entry);
                            }
                        } }
                }
                Toggle {
                    value: fullscreen,
                    label: "Fullscreen",
                    hint: FULLSCREEN_HINT.to_owned(),
                    onchange: {
                        let platform = platform.clone();

                        move |value: bool| {
                            if let Err(fault) = platform.host().set_fullscreen(value) {
                                warn!("Fullscreen could not be switched: {fault}");
                            }
                        }
                    },
                }
                Toggle {
                    value: logging,
                    label: "Enable Logging",
                    hint: LOGGING_HINT.to_owned(),
                    onchange: {
                        let platform = platform.clone();

                        move |value: bool| {
                            platform.host().remember(LOGGING_KEY, if value { LOGGING_ON } else { LOGGING_OFF });
                            logged.set(value);
                        }
                    },
                }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Language Priority" }
                button { class: "btn btn-danger btn-status", onclick: move |_| relocalize(&for_reset, settings, library, catalogs, &pictures_reset, &rigs_reset, lang::default_priority()), "Set to Default" }
                div { class: "card-outlined language-list",
                    for (index, code) in priority.iter().enumerate() {
                        {
                            let weak = none_index.is_some_and(|none| index > none);
                            let bold = Some(index) == none_index;
                            let label = lang::get_label_for_code(code);
                            let class = match (bold, weak) {
                                (true, _) => "language-label bold",
                                (_, true) => "language-label weak",
                                _ => "language-label",
                            };
                            let row_class = if dragging == Some(index) { "language-row dragging" } else { "language-row" };
                            let mut lifted = release.clone();
                            let mut dropped = release.clone();

                            rsx! {
                                div { key: "{code}", class: "{row_class}",
                                    span {
                                        class: "drag-handle symbols",
                                        onpointerdown: move |event| {
                                            let point = event.data().client_coordinates();
                                            let (_, y) = turn.point(point.x, point.y);

                                            event.prevent_default();
                                            drawers.claim(event.data().pointer_id());
                                            working.set(Some(settings.read().general.language_priority.clone()));
                                            drag.set(Drag::Pressed { index, last: y });
                                        },
                                        onpointermove: move |event| {
                                            if dragging_now(drag) {
                                                let point = event.data().client_coordinates();

                                                follow(point.x, point.y);
                                            }
                                        },
                                        onpointerup: move |_| {
                                            if dragging_now(drag) {
                                                lifted();
                                            }
                                        },
                                        onpointercancel: move |_| {
                                            if dragging_now(drag) {
                                                dropped();
                                            }
                                        },
                                        "\u{2630}"
                                    }
                                    span { class: "{class}", "{label}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Treasures" }
                Treasures {
                    config: settings.read().general.treasures.clone(),
                    catalog: globals.as_ref().map(|globals| Treasury(Arc::clone(&globals.0.catalog))),
                    onarc: { let platform = platform.clone(); move |(arc, value): (usize, String)| apply(&platform, settings, |next| next.general.treasures.set_arc(arc, value)) },
                    onlevel: { let platform = platform.clone(); move |(slot, index, level): (usize, usize, i32)| apply(&platform, settings, |next| next.general.treasures.set_level(slot, index, level)) },
                }
            }
        }
    }
}

#[component]
pub(crate) fn Treasures(config: TreasureConfig, catalog: Option<Treasury>, onarc: EventHandler<(usize, String)>, onlevel: EventHandler<(usize, usize, i32)>) -> Element {
    let mut open = use_signal(HashSet::<usize>::new);
    let placeholder = FULL_PERCENT.to_string();

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-subsection",
                div { class: "subsection-title", "General" }
                for (arc, name) in ARCS.iter().enumerate() {
                    {
                        let entry = config.arcs[arc].clone();
                        let tuned = config.is_tuned(arc);

                        rsx! {
                            label { class: "entry-row",
                                span { class: "entry-label", "{name}" }
                                input { class: "input entry-input", r#type: "text", inputmode: "numeric", placeholder: "{placeholder}", value: "{entry}",
                                    oninput: move |event| {
                                        let value = event.value();

                                        if typable_percent(&value) {
                                            onarc.call((arc, value));
                                        }
                                    } }
                                if tuned {
                                    span { class: "note weak", "Adjusted per treasure below" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "settings-subsection",
                div { class: "subsection-title", "Advanced" }
                for (arc, slots) in ARC_SLOTS.iter().enumerate() {
                    for (chapter, slot) in slots.iter().copied().enumerate() {
                        {
                            let expanded = open.read().contains(&slot);
                            let title = CHAPTER_TITLES[arc][chapter];

                            rsx! {
                                div { class: "fold",
                                    button { class: "btn btn-text fold-head", onclick: move |_| {
                                        let mut held = open.write();

                                        if !held.remove(&slot) {
                                            held.insert(slot);
                                        }
                                    },
                                        span { class: "bold fold-title", "{title}" }
                                        if !expanded {
                                            span { class: "note weak", "(collapsed)" }
                                        }
                                    }
                                    div { class: "rule" }
                                    if expanded {
                                        match catalog.as_ref() {
                                            Some(catalog) => chapter_view(&config, &catalog.0, slot, onlevel),
                                            None => rsx! { div { class: "note weak", "Treasure data has not loaded yet" } },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn chapter_view(config: &TreasureConfig, catalog: &Catalog, slot: usize, onlevel: EventHandler<(usize, usize, i32)>) -> Element {
    let groups = catalog.groups(slot);

    if groups.is_empty() {
        return rsx! { div { class: "note weak", "No treasure data was found for this chapter" } };
    }

    let grades = catalog.grades.clone();

    rsx! {
        div { class: "treasure-sets",
            for (group, set) in groups.iter().enumerate() {
                {
                    let members: Vec<i32> = set.members().collect();

                    if members.is_empty() {
                        rsx! {}
                    } else {
                        let name = catalog.set(slot, group).map_or_else(|| format!("Set {}", group + 1), str::to_owned);
                        let effect = catalog.effect(slot, group).map(str::to_owned);

                        rsx! {
                            div { class: "treasure-set",
                                div { class: "treasure-set-title",
                                    span { class: "bold", "{name}" }
                                    if let Some(effect) = effect {
                                        span { class: "note weak", "{effect}" }
                                    }
                                }
                                for stage in members {
                                    if let Ok(index) = usize::try_from(stage) {
                                        {
                                            let label = catalog.item(slot, stage).map_or_else(|| format!("Treasure {}", stage + 1), str::to_owned);
                                            let level = config.level(slot, index);
                                            let selected = usize::try_from(level).ok().and_then(|level| grades.get(level)).cloned().unwrap_or_default();
                                            let grades = grades.clone();

                                            rsx! {
                                                label { class: "treasure-row",
                                                    span { class: "treasure-label", "{label}" }
                                                    select { class: "combo", value: "{selected}",
                                                        onchange: move |event| {
                                                            let level = grades.iter().position(|grade| *grade == event.value()).unwrap_or(0) as i32;

                                                            onlevel.call((slot, index, level));
                                                        },
                                                        for grade in catalog.grades.iter() {
                                                            option { value: "{grade}", selected: *grade == selected, "{grade}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Cats(platform: Platform, settings: Signal<GameSettings>) -> Element {
    let held = settings.read();
    let cat_data = held.cat_data.clone();
    let level_fallback = CatDataSettings::default().default_level;
    let mut level_buffer = use_signal(|| if cat_data.default_level == level_fallback { String::new() } else { cat_data.default_level.to_string() });
    let p = [platform.clone(), platform.clone(), platform.clone(), platform.clone(), platform.clone(), platform.clone()];

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "Cat List" }
                label { class: "setting-row inline",
                    span { "Preferred Banner Form" }
                    select { class: "combo", value: "{cat_files::form_name(cat_data.preferred_banner_form)}",
                        onchange: {
                            let platform = p[0].clone();
                            move |event| {
                                if let Some(form) = (0..cat_files::FORM_COUNT).find(|form| cat_files::form_name(*form) == event.value()) {
                                    apply(&platform, settings, |next| next.cat_data.preferred_banner_form = form);
                                }
                            }
                        },
                        for form in 0..cat_files::FORM_COUNT {
                            option { value: "{cat_files::form_name(form)}", selected: form == cat_data.preferred_banner_form, "{cat_files::form_name(form)}" }
                        }
                    }
                }
                Toggle { value: cat_data.show_invalid_cats, label: "Show Invalid Cats", onchange: { let platform = p[1].clone(); move |value| apply(&platform, settings, |next| next.cat_data.show_invalid_cats = value) } }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Ability Display" }
                Toggle { value: cat_data.expand_spirit_details, label: "Expand Spirit Details by Default", onchange: { let platform = p[2].clone(); move |value| apply(&platform, settings, |next| next.cat_data.expand_spirit_details = value) } }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Level Display" }
                label { class: "setting-row inline",
                    span { "Default Level" }
                    input { class: "input entry-input", r#type: "text", inputmode: "numeric", placeholder: "{level_fallback}", value: "{level_buffer}", disabled: cat_data.auto_level_calculations,
                        oninput: {
                            let platform = p[3].clone();
                            move |event| {
                                let value = event.value();

                                if value.chars().all(|glyph| glyph.is_ascii_digit()) {
                                    level_buffer.set(value.clone());

                                    if value.is_empty() {
                                        apply(&platform, settings, |next| next.cat_data.default_level = level_fallback);
                                    } else if let Ok(parsed) = value.parse::<i32>() {
                                        apply(&platform, settings, |next| next.cat_data.default_level = parsed.max(1));
                                    }
                                }
                            }
                        } }
                }
                Toggle {
                    value: cat_data.auto_level_calculations,
                    label: "Auto Level Calculations",
                    hint: "Automatically calculates the max reasonable level for a unit based on their level caps",
                    onchange: { let platform = p[4].clone(); move |value| apply(&platform, settings, |next| next.cat_data.auto_level_calculations = value) },
                }
                Toggle {
                    value: cat_data.bump_ultra_60,
                    label: "Lv60 For Ultra",
                    hint: "Automatically bumps the level to 60 (if not higher already) when an Ultra Form or Ultra Talent is selected",
                    onchange: { let platform = p[5].clone(); move |value| apply(&platform, settings, |next| next.cat_data.bump_ultra_60 = value) },
                }
            }
        }
    }
}

#[component]
fn Stages(platform: Platform, settings: Signal<GameSettings>) -> Element {
    let current = settings.read().stages.sidebar_behavior.clone();
    let label = SIDEBAR_BEHAVIORS.iter().find(|(behavior, _)| *behavior == current).map_or("Cover", |(_, label)| label);

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "Stage List" }
                label { class: "setting-row",
                    span { "Sidebar Behavior" }
                    select { class: "combo", value: "{label}",
                        onchange: move |event| {
                            let chosen = SIDEBAR_BEHAVIORS.iter().find(|(_, label)| *label == event.value()).map(|(behavior, _)| behavior.clone());

                            if let Some(behavior) = chosen {
                                apply(&platform, settings, |next| next.stages.sidebar_behavior = behavior);
                            }
                        },
                        for (_, option) in SIDEBAR_BEHAVIORS {
                            option { value: "{option}", selected: option == label, "{option}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Enemies(platform: Platform, settings: Signal<GameSettings>) -> Element {
    let show_invalid = settings.read().enemy_data.show_invalid_enemies;

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "Enemy List" }
                Toggle { value: show_invalid, label: "Show Invalid Enemies", onchange: move |value| apply(&platform, settings, |next| next.enemy_data.show_invalid_enemies = value) }
            }
        }
    }
}

#[component]
fn Override(label: &'static str, hint: &'static str, fallback: i32, held: i32, signed: bool, onchange: EventHandler<i32>) -> Element {
    let mut buffer = use_signal(|| if held == fallback { String::new() } else { held.to_string() });

    rsx! {
        label { class: "setting-row inline",
            Tip { text: hint, hover: true, span { "{label}" } }
            input { class: "input entry-input", r#type: "text", inputmode: "numeric", placeholder: "{fallback}", value: "{buffer}",
                oninput: move |event| {
                    let typed = event.value();
                    let digits = typed.strip_prefix('-').filter(|_| signed).unwrap_or(&typed);

                    if !digits.chars().all(|glyph| glyph.is_ascii_digit()) {
                        return;
                    }

                    buffer.set(typed.clone());

                    if typed.is_empty() {
                        onchange.call(fallback);
                    } else if let Ok(parsed) = typed.parse::<i32>() {
                        onchange.call(parsed);
                    }
                } }
        }
    }
}

#[component]
fn AnimationTab(platform: Platform, settings: Signal<GameSettings>) -> Element {
    let held = settings.read().animation.clone();
    let fallback = AnimSettings::default();
    let positions = held.combat_hit_positions;
    let p = [platform.clone(), platform.clone(), platform.clone(), platform.clone(), platform.clone()];

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "Combat" }
                label { class: "setting-row inline",
                    Tip { text: positions.hint(), hover: true, span { "Hit Positions" } }
                    select { class: "combo", value: "{positions}",
                        onchange: {
                            let platform = p[0].clone();
                            move |event| {
                                if let Some(picked) = HitPositions::ALL.into_iter().find(|held| held.to_string() == event.value()) {
                                    apply(&platform, settings, |next| next.animation.combat_hit_positions = picked);
                                }
                            }
                        },
                        for choice in HitPositions::ALL {
                            option { value: "{choice}", selected: choice == positions, "{choice}" }
                        }
                    }
                }
                Override { label: "Hit Fade", hint: HIT_FADE_HINT, fallback: fallback.combat_hit_fade, held: held.combat_hit_fade, signed: false,
                    onchange: { let platform = p[1].clone(); move |value: i32| apply(&platform, settings, |next| next.animation.combat_hit_fade = value) } }
                Override { label: "System Height", hint: SYSTEM_HEIGHT_HINT, fallback: fallback.combat_height, held: held.combat_height, signed: false,
                    onchange: { let platform = p[2].clone(); move |value: i32| apply(&platform, settings, |next| next.animation.combat_height = value.max(COMBAT_HEIGHT_FLOOR)) } }
                Override { label: "System Offset", hint: SYSTEM_OFFSET_HINT, fallback: fallback.combat_offset, held: held.combat_offset, signed: true,
                    onchange: { let platform = p[3].clone(); move |value: i32| apply(&platform, settings, |next| next.animation.combat_offset = value) } }
                Override { label: "Fill Opacity", hint: FILL_OPACITY_HINT, fallback: fallback.combat_opacity, held: held.combat_opacity, signed: false,
                    onchange: { let platform = p[4].clone(); move |value: i32| apply(&platform, settings, |next| next.animation.combat_opacity = value.min(COMBAT_OPACITY_CEILING)) } }
                for layer in CombatLayer::ALL {
                    {
                        let platform = platform.clone();

                        rsx! {
                            Toggle { key: "{layer.label()}", value: held.combat_layer(layer), label: layer.label(), onchange: move |value| apply(&platform, settings, |next| next.animation.set_combat_layer(layer, value)) }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SandboxTab(platform: Platform, settings: Signal<GameSettings>) -> Element {
    let held = settings.read();
    let banner_form = held.sandbox.banner_form;
    let disable_replays = held.sandbox.disable_replays;
    let replay_source = held.sandbox.replay_source;
    let keys = held.sandbox.keys.clone();

    drop(held);

    let mut capturing: Signal<Option<Bind>> = use_signal(|| None);
    let listening = capturing();
    let form_platform = platform.clone();
    let reset_platform = platform.clone();
    let capture_platform = platform.clone();
    let replay_platform = platform.clone();
    let source_platform = platform.clone();

    rsx! {
        div { class: "settings-sections",
            div { class: "settings-section",
                div { class: "settings-heading", "Lineup List" }
                label { class: "setting-row inline",
                    span { "Lineup Banner Form" }
                    select { class: "combo", value: "{cat_files::form_name(banner_form)}",
                        onchange: move |event| {
                            if let Some(form) = (0..cat_files::FORM_COUNT).find(|form| cat_files::form_name(*form) == event.value()) {
                                apply(&form_platform, settings, |next| next.sandbox.banner_form = form);
                            }
                        },
                        for form in 0..cat_files::FORM_COUNT {
                            option { value: "{cat_files::form_name(form)}", selected: form == banner_form, "{cat_files::form_name(form)}" }
                        }
                    }
                }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Replay" }
                label { class: "setting-row inline",
                    Tip { text: REPLAY_SOURCE_HINT, hover: true, span { "Replay Source" } }
                    select { class: "combo", value: "{replay_source}",
                        onchange: move |event| {
                            if let Some(picked) = ReplaySource::ALL.into_iter().find(|source| source.to_string() == event.value()) {
                                apply(&source_platform, settings, |next| next.sandbox.replay_source = picked);
                            }
                        },
                        for source in ReplaySource::ALL {
                            option { value: "{source}", selected: source == replay_source, "{source}" }
                        }
                    }
                }
                Toggle {
                    value: disable_replays,
                    label: "Disable Replays",
                    hint: "Disables the replay set-up path\nMay improve battle start speeds",
                    onchange: move |value| apply(&replay_platform, settings, |next| next.sandbox.disable_replays = value),
                }
            }
            div { class: "settings-section",
                div { class: "settings-heading", "Keybinds" }
                button { class: "btn btn-danger btn-status", onclick: move |_| apply(&reset_platform, settings, |next| next.sandbox.keys.reset()), "Set to Default" }
                div {
                    class: "keybind-list",
                    tabindex: "0",
                    onkeydown: move |event| {
                        let Some(bind) = *capturing.peek() else {
                            return;
                        };

                        event.prevent_default();

                        if event.data().is_auto_repeating() {
                            return;
                        }

                        if let Some(name) = key_name(&event.data().key()) {
                            apply(&capture_platform, settings, |next| next.sandbox.keys.set(bind, &name));
                        }

                        capturing.set(None);
                    },
                    onfocusout: move |_| capturing.set(None),
                    for bind in Bind::ALL {
                        {
                            let held = keys.key(bind);
                            let waiting = listening == Some(bind);
                            let shown = match (waiting, held.is_empty()) {
                                (true, _) => KEY_PROMPT.to_owned(),
                                (false, true) => KEY_UNBOUND.to_owned(),
                                (false, false) => key_label(held),
                            };
                            let class = if waiting { "btn btn-success keybind-btn" } else { "btn btn-neutral keybind-btn" };

                            rsx! {
                                div { key: "{bind.id()}", class: "keybind-row",
                                    span { class: "keybind-label", "{bind.label()}" }
                                    button { class: "{class}", onclick: move |_| capturing.set(Some(bind)), "{shown}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
