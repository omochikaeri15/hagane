mod words;

use std::rc::Rc;

use dioxus::prelude::*;

use foundry::domains::cat::combo::{self, CatCombo};
use foundry::domains::sandbox::Lineup;

use crate::platform::Platform;
use crate::systems::context::Shared;
use crate::systems::images;
use crate::systems::picture::Pic;
use crate::widget::{FitColumn, RowList, drawer};

use super::{Cats, Ruled};

const DOUBLE_CLICK_MS: f64 = 400.0;
const ROW_HEIGHT: f32 = 58.0;
const ICON_HEIGHT: f32 = 52.0;
const ICON_GAP: f32 = 3.0;
const SLOT_ASPECT: f32 = 110.0 / 85.0;
const TEXT_WIDTH: f32 = 170.0;
const ROW_PADDING: f32 = 8.0;
const BODY_GAP: f32 = 8.0;
const SMALLEST: f32 = 0.7;
const TEXT_SMALLEST: f32 = 0.8;
const BLOCK_FLOOR: f32 = 60.0;
const NAME_SIZE: f32 = 13.0;
const EFFECT_SIZE: f32 = 12.0;
const RESTRICTION_SIZE: f32 = 10.0;
const FIT_FLOOR: f32 = 7.0;
const LINE_GAP: f32 = 1.0;
const PLAY_CLEARANCE: f32 = 60.0;
const LOOKAHEAD_ROWS: usize = 12;
const EVERY_KIND: &str = "All Types";
const ANY_STRENGTH: &str = "Any Strength";
const ACTIVE: &str = "Active";
const INACTIVE: &str = "Inactive";
const BANNED_NOTE: &str = "Disabled on this stage";
const SLOT_COUNT: usize = 5;
const FILTER_SIZE: f32 = 12.0;
const GLYPH_SHARE: f32 = 0.62;
const PICK_CHROME: f32 = 40.0;
const TITLE_WIDTH: f32 = 70.0;
const SEARCH_FLOOR: f32 = 90.0;
const HEADER_GAP: f32 = 8.0;
const PHONE_BLOCK: f32 = 135.0;
const PHONE_WIDEST: f32 = 320.0;
const PHONE_PAD: f32 = 4.0;
const LINE_HEIGHT: f32 = 1.3;
const BASE_LINES: f32 = 3.0;
const SAMPLES: usize = 8;
const SAMPLE_GLYPHS: f32 = 0.52;
const SAMPLE_NAME: f32 = 16.0;
const SAMPLE_EFFECT: f32 = 14.0;

#[derive(Clone)]
pub(crate) struct Held(pub(crate) Rc<Lineup>);

impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct Card {
    combo: CatCombo,
    folded: String,
}

#[derive(Clone, PartialEq, Default)]
enum Strength {
    #[default]
    Any,
    Active,
    Inactive,
    Band(i32),
}

struct Deck {
    cards: Vec<Card>,
    kinds: Vec<(i32, String)>,
    bands: Vec<(i32, String)>,
    kind_width: f32,
    strength_width: f32,
}

fn widest<'a>(labels: impl Iterator<Item = &'a str>) -> f32 {
    labels.map(|label| label.chars().count()).max().unwrap_or(0) as f32 * FILTER_SIZE * GLYPH_SHARE + PICK_CHROME
}

#[derive(Clone)]
struct Decked(Rc<Deck>);

impl PartialEq for Decked {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy)]
struct Fit {
    row: f32,
    slot: (f32, f32),
    block: f32,
    text: f32,
    scale: f32,
}

impl Fit {
    fn of(width: f32) -> Self {
        let width = if width > 0.0 { width } else { f32::MAX };
        let slots = SLOT_COUNT as f32;
        let gaps = ICON_GAP * (slots - 1.0);
        let spare = width - ROW_PADDING * 2.0 - BODY_GAP;
        let scale = ((spare - gaps) / (ICON_HEIGHT * SLOT_ASPECT * slots + TEXT_WIDTH)).clamp(SMALLEST, 1.0);
        let slot = (ICON_HEIGHT * SLOT_ASPECT * scale, ICON_HEIGHT * scale);
        let block = (spare - slot.0 * slots - gaps).min(TEXT_WIDTH * scale).max(BLOCK_FLOOR);

        Self { row: ROW_HEIGHT * scale, slot, block, text: scale.max(TEXT_SMALLEST), scale }
    }

    fn phone(width: f32) -> Self {
        let width = if width > 0.0 { width } else { f32::MAX };
        let slots = SLOT_COUNT as f32;
        let chrome = ROW_PADDING * 2.0 + BODY_GAP + ICON_GAP * (slots - 1.0) + ICON_HEIGHT * SLOT_ASPECT * slots;
        let scale = (width / (chrome + PHONE_BLOCK)).clamp(0.0, 1.0);
        let block = if scale < 1.0 { PHONE_BLOCK * scale } else { (width - chrome).min(PHONE_WIDEST) };

        Self { row: ROW_HEIGHT * scale, slot: (ICON_HEIGHT * SLOT_ASPECT * scale, ICON_HEIGHT * scale), block, text: scale, scale }
    }
}

fn estimated_lines(text: &str, size: f32) -> usize {
    let glyphs: usize = text.chars().map(|glyph| if glyph.is_ascii() { 1 } else { 2 }).sum();

    (glyphs as f32 * size * SAMPLE_GLYPHS / PHONE_BLOCK).ceil() as usize
}

fn note_of(card: &Card, banned: bool) -> String {
    if banned { BANNED_NOTE.to_owned() } else { card.combo.restriction.clone().unwrap_or_default() }
}

fn tallest_looking(deck: &Deck, rules: &Ruled) -> Vec<(String, String, String)> {
    let weight = |card: &Card| {
        let lines = estimated_lines(&card.combo.name, SAMPLE_NAME) + estimated_lines(&card.combo.effect, SAMPLE_EFFECT);

        (lines, card.combo.name.chars().count() + card.combo.effect.chars().count())
    };
    let noted: Vec<(&Card, String)> = deck.cards.iter().map(|card| (card, note_of(card, rules.0.bans(card.combo.effect_type)))).collect();
    let (mut with, mut without): (Vec<_>, Vec<_>) = noted.into_iter().partition(|(_, note)| !note.is_empty());

    with.sort_by_key(|(card, _)| std::cmp::Reverse(weight(card)));
    without.sort_by_key(|(card, _)| std::cmp::Reverse(weight(card)));

    with.into_iter()
        .take(SAMPLES / 2)
        .chain(without.into_iter().take(SAMPLES / 2))
        .map(|(card, note)| (card.combo.name.clone(), card.combo.effect.clone(), note))
        .collect()
}

fn build(globals: &Shared, cats: &Cats) -> Deck {
    let cards: Vec<Card> = combo::every(globals.0.context(), &cats.0).into_iter().map(|combo| Card { folded: combo.name.to_lowercase(), combo }).collect();
    let mut kinds: Vec<(i32, String)> = Vec::new();
    let mut bands: Vec<(i32, String)> = Vec::new();

    for card in &cards {
        if !kinds.iter().any(|(effect, _)| *effect == card.combo.effect_type) {
            kinds.push((card.combo.effect_type, card.combo.kind.clone()));
        }

        if !card.combo.band.is_empty() && !bands.iter().any(|(level, _)| *level == card.combo.effect_level) {
            bands.push((card.combo.effect_level, card.combo.band.clone()));
        }
    }

    kinds.sort_by_key(|(effect, _)| *effect);
    bands.sort_by_key(|(level, _)| *level);

    let kind_width = widest(std::iter::once(EVERY_KIND).chain(kinds.iter().map(|(_, label)| label.as_str())));
    let strength_width = widest([ANY_STRENGTH, ACTIVE, INACTIVE].into_iter().chain(bands.iter().map(|(_, label)| label.as_str())));

    Deck { cards, kinds, bands, kind_width, strength_width }
}

#[component]
pub(crate) fn Combos(cats: Cats, globals: Shared, rules: Ruled, lineup: Held, focus: Option<(u32, usize)>, onadopt: EventHandler<usize>) -> Element {
    let platform = use_context::<Platform>();
    let pictures = images::pictures();
    let mut query = use_signal(String::new);
    let mut kind: Signal<Option<i32>> = use_signal(|| None);
    let mut strength = use_signal(Strength::default);
    let mut clicked: Signal<Option<(usize, f64)>> = use_signal(|| None);
    let mut across = use_signal(|| 0.0f32);
    let drawers = drawer::drawers();
    let phone = (drawers.compact)();
    let clearance = if phone { 0.0 } else { PLAY_CLEARANCE };
    let fit = if phone { Fit::phone(across()) } else { Fit::of(across()) };
    let shared = globals.clone();
    let deck = use_memo(use_reactive!(|(cats, shared)| Decked(Rc::new(build(&shared, &cats)))));
    let deck = deck();
    let sampled = deck.clone();
    let ruling = rules.clone();
    let samples = use_memo(use_reactive!(|(sampled, ruling)| Rc::new(tallest_looking(&sampled.0, &ruling))));
    let samples = samples();
    let mut needs: Signal<[f32; SAMPLES]> = use_signal(|| [0.0; SAMPLES]);
    let tallest = needs().into_iter().fold(0.0, f32::max);
    let height = if phone { (BASE_LINES * words::base_line(fit.scale) * LINE_HEIGHT).max(tallest) + PHONE_PAD * fit.scale } else { fit.row };
    let rows = {
        let vault = globals.0.vault();

        vault.vds.cats.combos(&vault.vfs)
    };
    let wanted = query.read().to_lowercase();
    let strength_now = strength();
    let kind_now = kind();
    let mut shown: Vec<(usize, bool, bool)> = deck
        .0
        .cards
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            let banned = rules.0.bans(card.combo.effect_type);
            let active = !banned && rows.get(card.combo.line).is_some_and(|row| lineup.0.active(row));
            let strong = match strength_now {
                Strength::Any => true,
                Strength::Active => active,
                Strength::Inactive => !active,
                Strength::Band(level) => card.combo.effect_level == level,
            };
            let joined = focus.is_none_or(|(unit, form)| card.combo.members.iter().any(|member| member.id == Some(unit) && member.form <= form));

            (joined && strong && kind_now.is_none_or(|effect| card.combo.effect_type == effect) && card.folded.contains(&wanted)).then_some((index, active, banned))
        })
        .collect();

    shown.sort_by(|left, right| {
        let (held_left, held_right) = (&deck.0.cards[left.0], &deck.0.cards[right.0]);

        right
            .1
            .cmp(&left.1)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| held_left.combo.effect_type.cmp(&held_right.combo.effect_type))
            .then_with(|| held_right.combo.effect_level.cmp(&held_left.combo.effect_level))
            .then_with(|| held_left.combo.name.cmp(&held_right.combo.name))
    });

    let shown = Rc::new(shown);
    let count = shown.len();
    let listed = Rc::clone(&shown);
    let cards = deck.clone();
    let kind_label = kind_now.and_then(|effect| deck.0.kinds.iter().find(|(held, _)| *held == effect)).map_or(EVERY_KIND, |(_, label)| label.as_str()).to_owned();
    let strength_label = match &strength_now {
        Strength::Any => ANY_STRENGTH.to_owned(),
        Strength::Active => ACTIVE.to_owned(),
        Strength::Inactive => INACTIVE.to_owned(),
        Strength::Band(level) => deck.0.bands.iter().find(|(held, _)| held == level).map_or_else(|| ANY_STRENGTH.to_owned(), |(_, label)| label.clone()),
    };
    let kinds = deck.clone();
    let bands = deck.clone();
    let room = across();
    let single = room <= 0.0 || room >= TITLE_WIDTH + SEARCH_FLOOR + deck.0.kind_width + deck.0.strength_width + HEADER_GAP * 3.0;
    let (kind_width, strength_width) = (deck.0.kind_width, deck.0.strength_width);
    let row = Callback::new(move |index: usize| {
        let Some((at, active, banned)) = listed.get(index).copied() else {
            return rsx! {};
        };
        let card = &cards.0.cards[at];
        let line = card.combo.line;
        let class = match (active, banned) {
            (_, true) => "combo-row banned",
            (true, false) => "combo-row active",
            (false, false) => "combo-row",
        };
        let note = note_of(card, banned);
        let (slot_width, slot_height) = fit.slot;
        let platform = platform.clone();
        let labels = if phone {
            rsx! { words::Words { name: card.combo.name.clone(), effect: card.combo.effect.clone(), note, scale: fit.scale, room: fit.block, tall: height } }
        } else {
            let mut lines = vec![(card.combo.name.clone(), NAME_SIZE * fit.text), (card.combo.effect.clone(), EFFECT_SIZE * fit.text)];

            if !note.is_empty() {
                lines.push((note, RESTRICTION_SIZE * fit.text));
            }

            rsx! { FitColumn { lines, gap: LINE_GAP, floor: FIT_FLOOR, bold_first: true } }
        };
        let frame = if phone { format!("height: {height}px; padding: 0 {}px; gap: {}px;", ROW_PADDING * fit.scale, BODY_GAP * fit.scale) } else { format!("height: {height}px;") };
        let spacing = if phone { format!("gap: {}px;", ICON_GAP * fit.scale) } else { String::new() };

        rsx! {
            div {
                key: "{line}",
                class: "{class}",
                style: "{frame}",
                onclick: move |_| {
                    let now = platform.host().now();
                    let doubled = clicked.peek().is_some_and(|(held, at)| held == line && now - at <= DOUBLE_CLICK_MS);

                    clicked.set((!doubled).then_some((line, now)));

                    if doubled {
                        onadopt.call(line);
                    }
                },
                div { class: "combo-labels", style: "width: {fit.block}px;", {labels} }
                div { class: "combo-icons", style: "{spacing}",
                    for (slot, member) in card.combo.members.iter().enumerate().take(SLOT_COUNT) {
                        {
                            let icon = member.icon.as_ref().and_then(|source| pictures.trimmed(&source.path));
                            let class = if icon.is_some() { "combo-icon" } else { "combo-icon empty" };

                            rsx! {
                                div { key: "{slot}", class, style: "width: {slot_width}px; height: {slot_height}px;",
                                    if let Some(icon) = icon {
                                        Pic { url: icon.url }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    rsx! {
        div { class: "outlined-card combos-card",
            div { class: if single { "combos-header single" } else { "combos-header" },
                div { class: "combos-find",
                    span { class: "bold combos-title", "Combos" }
                    input { class: "input combos-search", r#type: "search", placeholder: "Name...", value: "{query}", oninput: move |event| query.set(event.value()) }
                }
                div { class: "combos-filters",
                    select { class: "combo combos-kind", style: "width: {kind_width}px;", value: "{kind_label}",
                        onchange: move |event| {
                            kind.set(kinds.0.kinds.iter().find(|(_, label)| *label == event.value()).map(|(effect, _)| *effect));
                        },
                        option { value: "{EVERY_KIND}", selected: kind_now.is_none(), "{EVERY_KIND}" }
                        for (effect, label) in deck.0.kinds.iter() {
                            option { value: "{label}", selected: kind_now == Some(*effect), "{label}" }
                        }
                    }
                    select { class: "combo combos-strength", style: "width: {strength_width}px;", value: "{strength_label}",
                        onchange: move |event| {
                            let value = event.value();
                            let picked = match value.as_str() {
                                ANY_STRENGTH => Strength::Any,
                                ACTIVE => Strength::Active,
                                INACTIVE => Strength::Inactive,
                                other => bands.0.bands.iter().find(|(_, label)| label == other).map_or(Strength::Any, |(level, _)| Strength::Band(*level)),
                            };

                            strength.set(picked);
                        },
                        option { value: "{ANY_STRENGTH}", selected: strength_now == Strength::Any, "{ANY_STRENGTH}" }
                        option { value: "{ACTIVE}", selected: strength_now == Strength::Active, "{ACTIVE}" }
                        option { value: "{INACTIVE}", selected: strength_now == Strength::Inactive, "{INACTIVE}" }
                        for (level, label) in deck.0.bands.iter() {
                            option { value: "{label}", selected: strength_now == Strength::Band(*level), "{label}" }
                        }
                    }
                }
            }
            RowList {
                count,
                row,
                empty: "No Combos Found!",
                height,
                tail: clearance,
                buffer: LOOKAHEAD_ROWS,
                onwidth: move |width: f32| {
                    if *across.peek() != width {
                        across.set(width);
                    }
                },
            }
            if phone {
                div { class: "words-samples",
                    for (index, (name, effect, note)) in samples.iter().cloned().enumerate() {
                        words::Words {
                            key: "{index}",
                            name,
                            effect,
                            note,
                            scale: fit.scale,
                            room: fit.block,
                            onneeded: move |needed: f32| {
                                needs.with_mut(|held| {
                                    if let Some(slot) = held.get_mut(index) {
                                        *slot = needed;
                                    }
                                });
                            },
                        }
                    }
                }
            }
        }
    }
}
