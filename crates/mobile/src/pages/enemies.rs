use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use std::sync::Arc;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use codex::combat::{AttrUnit, Entity, Identity, REGISTRY};
use foundry::Vault;
use foundry::domains::enemy::filter::evaluation::{entity_passes_filter, get_identity_name};
use foundry::domains::enemy::filter::{ATTACK_TYPE_IDENTITIES, EnemyFilterState, MatchMode};
use foundry::domains::enemy::scanner::{self, EnemyEntry};
use foundry::domains::enemy::statblock::{self as enemy_statblock, Subject};
use foundry::systems::combat::abilities::collect_ability_data;
use foundry::systems::combat::registry::{
    DisplayGroup, Magnification, STAT_ATK_CYCLE, STAT_ATTACK, STAT_CASH_DROP, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS, STAT_RANGE, STAT_SPEED, StatContext, get_display_def,
};
use foundry::systems::combat::{RenderContext, present_identities};

use crate::catalog::Load;
use crate::context::Globals;
use crate::images::{self, Images};
use crate::pages::cats::{ability_rows, frames_cell, headers, icon_of, icon_row, name_size, value_cell};
use crate::pages::detail::{self, EXPORT_IDLE};
use crate::pages::Context;
use crate::roster::{Entry, Roster};
use crate::{AppWindow, Enemies, FilterIcon, FilterLabeled, HeaderBtn, Page, RangeField, Shell, Tone};

const NAME_WRAP: f32 = 120.0;
const TAB_LABELS: [&str; 3] = ["Abilities", "Details", "Animation"];
const EMPTY_ICON: &str = "uni.png";
const DEFAULT_MAGNIFICATION: &str = "100";
const ICON_BOX_WIDTH: f32 = 110.0;
const ICON_BOX_HEIGHT: f32 = 96.0;
pub const STAT_KEYS: [&str; 8] = ["Attack", "Dps", "Range", "Atk Cycle (f)", "Hitpoints", "Knockbacks", "Speed", "Cash Drop"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Abilities,
    Details,
    Animation,
}

impl Tab {
    fn index(self) -> i32 {
        match self {
            Self::Abilities => 0,
            Self::Details => 1,
            Self::Animation => 2,
        }
    }

    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Details,
            2 => Self::Animation,
            _ => Self::Abilities,
        }
    }
}

pub struct State {
    enemies: Option<Arc<Vec<EnemyEntry>>>,
    selected: Option<u32>,
    arriving: Option<u32>,
    tab: Tab,
    mag_input: String,
    magnification: Magnification,
    search: String,
    filter: EnemyFilterState,
    stats: HashMap<u32, Option<Entity>>,
    available: Option<Rc<HashSet<Identity>>>,
    roster: Rc<Roster>,
    exporting: bool,
}

pub type Shared = Rc<RefCell<State>>;

fn typable_magnification(value: &str) -> bool {
    value.chars().all(|glyph| glyph.is_ascii_digit() || matches!(glyph, '/' | '|' | '\\'))
}

pub fn parse_magnification(input: &str) -> Magnification {
    let trimmed = input.trim();
    let parts: Vec<&str> = trimmed.split(['/', '|', '\\']).collect();

    if parts.len() >= 2 {
        let hitpoints = parts[0].trim().parse::<i32>().unwrap_or(100);
        let attack = parts[1].trim().parse::<i32>().unwrap_or(hitpoints);

        Magnification { hitpoints, attack }
    } else {
        let magnification = trimmed.parse::<i32>().unwrap_or(100);

        Magnification { hitpoints: magnification, attack: magnification }
    }
}

fn matches(enemy: &EnemyEntry, query: &str) -> bool {
    let by_id = query.chars().next().is_some_and(|glyph| glyph.is_ascii_digit());

    (by_id && enemy.id_str().to_lowercase().contains(query)) || enemy.name.to_lowercase().contains(query)
}

fn identity_code(identity: Identity) -> i32 {
    REGISTRY.iter().position(|def| def.identity == identity).map_or(-1, |index| index as i32)
}

pub fn identity_from_code(code: i32) -> Option<Identity> {
    REGISTRY.get(usize::try_from(code).ok()?).map(|def| def.identity)
}

pub fn ability_schema(identity: Identity) -> &'static [(&'static str, AttrUnit)] {
    REGISTRY.iter().find(|def| def.identity == identity).map_or(&[], |def| def.schema)
}

fn group_identities(available: &HashSet<Identity>, target: DisplayGroup) -> Vec<Identity> {
    let mut found = Vec::new();

    for def in REGISTRY.iter() {
        if !available.contains(&def.identity) {
            continue;
        }

        let display = get_display_def(def.identity);

        if display.group != target || display.group == DisplayGroup::Trait || ATTACK_TYPE_IDENTITIES.contains(&def.identity) || found.contains(&def.identity) {
            continue;
        }

        found.push(def.identity);
    }

    found
}

impl State {
    fn new(images: &Images) -> Self {
        let roster = Roster::new(images, Rc::new(|images: &Images, vault: &Arc<Vault>, path: &Path| (format!("enemy-banner:{}", path.display()), images.enemy_banner(vault, path))));

        Self {
            enemies: None,
            selected: None,
            arriving: None,
            tab: Tab::Abilities,
            mag_input: DEFAULT_MAGNIFICATION.to_owned(),
            magnification: Magnification::default(),
            search: String::new(),
            filter: EnemyFilterState::default(),
            stats: HashMap::new(),
            available: None,
            roster,
            exporting: false,
        }
    }

    fn enemy(&self, id: u32) -> Option<EnemyEntry> {
        self.enemies.as_ref()?.iter().find(|enemy| enemy.id == id).cloned()
    }

    fn shown(&self) -> Vec<usize> {
        let Some(enemies) = &self.enemies else {
            return Vec::new();
        };
        let query = self.search.to_lowercase();

        enemies
            .iter()
            .enumerate()
            .filter(|(_, enemy)| entity_passes_filter(enemy, &self.filter))
            .filter(|(_, enemy)| query.is_empty() || matches(enemy, &query))
            .map(|(index, _)| index)
            .collect()
    }

    fn stats_of(&mut self, vault: &Vault, id: u32, show_invalid: bool) -> Option<Entity> {
        self.stats.entry(id).or_insert_with(|| scanner::scan_single(id, vault, show_invalid).map(|entry| entry.stats)).clone()
    }
}

fn tooltip(enemy: &EnemyEntry) -> String {
    format!("[ID] {}\n[Name] {}", enemy.id_str(), enemy.display_name())
}

fn set_export_face(ui: &AppWindow, label: &str, tone: Tone) {
    let enemies = ui.global::<Enemies>();

    enemies.set_export_label(label.into());
    enemies.set_export_tone(tone);
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new(&context.images)));
    let enemies = window.global::<Enemies>();

    enemies.set_roster(ModelRc::from(Rc::clone(&state.borrow().roster) as Rc<dyn Model<Data = crate::RosterRow>>));
    enemies.set_tabs(ModelRc::new(VecModel::from(TAB_LABELS.iter().map(|label| HeaderBtn { label: (*label).into(), available: true }).collect::<Vec<_>>())));
    enemies.set_export_label(EXPORT_IDLE.into());

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.catalogs.listen(Rc::new(move |ui: &AppWindow| sync(ui, &held, &ctx)));
    }

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.images.watch(Rc::new(move |ui: &AppWindow, key: &str| {
            if (key.starts_with("trim:") || key.starts_with("cut:")) && held.borrow().selected.is_some() && ui.global::<Shell>().get_page() == Page::Enemies {
                refresh(ui, &held, &ctx);
            }
        }));
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_select(move |id| {
        if let Some(ui) = weak.upgrade() {
            select(&ui, &held, &ctx, id.max(0) as u32);
        }
    });

    let (held, _, weak) = on(&state, context, window);
    enemies.on_search_changed(move |text| {
        held.borrow_mut().search = text.to_string();

        let shown = held.borrow().shown();

        held.borrow().roster.show(shown.clone());

        if let Some(ui) = weak.upgrade() {
            ui.global::<Enemies>().set_roster_count(shown.len() as i32);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_open_filter(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_filter(&ui, &held, &ctx);
            ui.global::<Enemies>().set_filter_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_set_tab(move |tab| {
        held.borrow_mut().tab = Tab::from_index(tab);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_mag_edited(move |value| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if typable_magnification(&value) {
            let mut state = held.borrow_mut();

            state.magnification = parse_magnification(&value);
            state.mag_input = value.to_string();
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_export_image(move || {
        if let Some(ui) = weak.upgrade() {
            export(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    enemies.on_appearances(move || {
        let Some(id) = held.borrow().selected else {
            return;
        };

        ctx.appearances.set(Some(id));

        if let Some(ui) = weak.upgrade() {
            ui.global::<Shell>().invoke_navigate(Page::Stages);
        }
    });

    wire_filter(window, &state, context);

    state
}

fn wire_filter(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let enemies = window.global::<Enemies>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    fn edited(ui: &AppWindow, held: &Shared, ctx: &Rc<Context>, change: impl FnOnce(&mut EnemyFilterState)) {
        change(&mut held.borrow_mut().filter);

        let shown = held.borrow().shown();

        held.borrow().roster.show(shown.clone());
        ui.global::<Enemies>().set_roster_count(shown.len() as i32);
        ui.global::<Enemies>().set_filter_active(held.borrow().filter.is_active());
        refresh_filter(ui, held, ctx);
    }

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_mode_set(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.match_mode = if index == 0 { MatchMode::And } else { MatchMode::Or });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_mag_edited(move |value| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.mag_input = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_stat_min(move |index, value| {
        if let Some(ui) = weak.upgrade()
            && let Some(stat) = STAT_KEYS.get(index.max(0) as usize)
        {
            edited(&ui, &held, &ctx, |filter| filter.stat_ranges.entry(stat).or_default().min = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_stat_max(move |index, value| {
        if let Some(ui) = weak.upgrade()
            && let Some(stat) = STAT_KEYS.get(index.max(0) as usize)
        {
            edited(&ui, &held, &ctx, |filter| filter.stat_ranges.entry(stat).or_default().max = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_icon(move |code| {
        if let (Some(ui), Some(identity)) = (weak.upgrade(), identity_from_code(code)) {
            edited(&ui, &held, &ctx, |filter| {
                if !filter.active_identities.remove(&identity) {
                    filter.active_identities.insert(identity);
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_range_min(move |code, index, value| {
        if let (Some(ui), Some(identity)) = (weak.upgrade(), identity_from_code(code))
            && let Some((attr, _)) = ability_schema(identity).get(index.max(0) as usize).copied()
        {
            edited(&ui, &held, &ctx, |filter| filter.adv_ranges.entry(identity).or_default().entry(attr).or_default().min = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_range_max(move |code, index, value| {
        if let (Some(ui), Some(identity)) = (weak.upgrade(), identity_from_code(code))
            && let Some((attr, _)) = ability_schema(identity).get(index.max(0) as usize).copied()
        {
            edited(&ui, &held, &ctx, |filter| filter.adv_ranges.entry(identity).or_default().entry(attr).or_default().max = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    enemies.on_filter_clear(move || {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| *filter = EnemyFilterState { is_open: filter.is_open, treasure: filter.treasure, ..EnemyFilterState::default() });
        }
    });
}

fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let enemies = ui.global::<Enemies>();
    let load = context.catalogs.enemies();

    match &load {
        Load::Ready(found) => {
            let fresh = state.borrow().enemies.as_ref().is_none_or(|held| !Arc::ptr_eq(held, found));

            if !fresh {
                return;
            }

            let entries: Vec<Entry> = found.iter().map(|enemy| Entry { id: enemy.id, banner: enemy.icon_path.clone(), tooltip: tooltip(enemy).into() }).collect();

            {
                let mut held = state.borrow_mut();

                held.enemies = Some(Arc::clone(found));
                held.stats.clear();
                held.available = None;
            }

            let shown = state.borrow().shown();

            state.borrow().roster.fill(context.engine.vault(), entries, shown.clone());
            enemies.set_roster_count(shown.len() as i32);
            enemies.set_ready(true);

            let remembered = context.memory.get();
            let wanted = state.borrow().selected.or(remembered.last_enemy).filter(|id| found.iter().any(|enemy| enemy.id == *id));

            match wanted {
                Some(id) => select(ui, state, context, id),
                None => {
                    enemies.set_selected(-1);
                    enemies.set_roster_open(true);
                }
            }
        }
        Load::Loading(..) | Load::Idle => {
            enemies.set_ready(false);
            enemies.set_status_title(if load.scanning() { "Scanning Enemies...".into() } else { "Loading Enemies...".into() });
            enemies.set_status_detail(load.detail().unwrap_or_default().into());
        }
    }
}

fn apply(state: &Shared, id: u32) {
    let mut held = state.borrow_mut();

    if held.selected != Some(id) {
        held.mag_input = DEFAULT_MAGNIFICATION.to_owned();
        held.magnification = Magnification::default();
    }

    held.selected = Some(id);
}

pub fn entered(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let shown = state.borrow().selected.and_then(|id| state.borrow().enemy(id));

    if shown.is_some() {
        refresh(ui, state, context);
    }
}

fn select(ui: &AppWindow, state: &Shared, context: &Rc<Context>, id: u32) {
    if state.borrow().enemy(id).is_none() {
        return;
    }

    let previous = state.borrow().selected.filter(|held| *held != id && state.borrow().enemy(*held).is_some());

    apply(state, id);
    context.memory.update(|memory| memory.last_enemy = Some(id));
    refresh(ui, state, context);

    let holdable = state.borrow().tab != Tab::Animation && context.images.pending() > 0;

    match previous {
        Some(shown) if holdable => {
            apply(state, shown);
            refresh(ui, state, context);
            state.borrow_mut().arriving = Some(id);

            let (held, ctx) = (Rc::clone(state), Rc::clone(context));

            context.images.settled(
                ui,
                Instant::now() + images::SETTLE_LIMIT,
                Rc::new(move |ui| {
                    if held.borrow().arriving != Some(id) {
                        return;
                    }

                    held.borrow_mut().arriving = None;
                    apply(&held, id);
                    refresh(ui, &held, &ctx);
                }),
            );
        }
        _ => state.borrow_mut().arriving = None,
    }
}

fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let enemies = ui.global::<Enemies>();
    let Some(globals) = context.globals() else {
        return;
    };
    let Some(enemy) = state.borrow().selected.and_then(|id| state.borrow().enemy(id)) else {
        enemies.set_selected(-1);
        return;
    };
    let vault = context.engine.vault();
    let images = &context.images;
    let (tab, mag_input, magnification) = {
        let held = state.borrow();

        (held.tab, held.mag_input.clone(), held.magnification)
    };
    let name = enemy.display_name();

    enemies.set_selected(enemy.id as i32);
    enemies.set_tab(tab.index());
    enemies.set_mag_input(mag_input.as_str().into());
    enemies.set_name(name.as_str().into());
    enemies.set_name_size(name_size(&name, NAME_WRAP));
    enemies.set_id_text(format!("ID: {:03}-E", enemy.id).into());

    let icon = enemy.icon_path.as_ref().and_then(|path| images.trimmed(&vault, path)).or_else(|| vault.vfs.find(EMPTY_ICON).and_then(|path| images.trimmed(&vault, &path)));

    if let Some(icon) = &icon {
        let (width, height) = Images::size_of(icon);
        let scale = (ICON_BOX_WIDTH / width.max(1) as f32).min(ICON_BOX_HEIGHT / height.max(1) as f32);

        enemies.set_icon_width(width as f32 * scale);
        enemies.set_icon_height(height as f32 * scale);
    }

    enemies.set_has_icon(icon.is_some());
    enemies.set_icon(icon.unwrap_or_default());

    match tab {
        Tab::Abilities => {
            let show_invalid = context.state.settings().show_invalid_enemies();
            let stats = state.borrow_mut().stats_of(&vault, enemy.id, show_invalid).unwrap_or_else(|| enemy.stats.clone());

            refresh_abilities(ui, &globals, &vault, images, &enemy, &stats, magnification);
            crate::viewer::hide(context);
        }
        Tab::Details => {
            let description = Some(enemy.description.join("\n")).filter(|text| !text.trim().is_empty()).unwrap_or_else(|| "No description available".to_owned());

            enemies.set_description(detail::lines(&description));
            crate::viewer::hide(context);
        }
        Tab::Animation => crate::viewer::show_enemy(ui, context, &enemy),
    }
}

fn refresh_abilities(ui: &AppWindow, globals: &Rc<Globals>, vault: &Arc<Vault>, images: &Images, enemy: &EnemyEntry, stats: &Entity, magnification: Magnification) {
    let enemies = ui.global::<Enemies>();
    let ctx = RenderContext::enemy(globals.context(vault), stats, magnification);
    let (traits, headline_1, headline_2, body_1, body_2, footer) = collect_ability_data(&ctx);
    let stat_ctx = StatContext::enemy(stats, enemy.atk_anim_frames, magnification, &globals.treasure);

    enemies.set_stat_headers_1(headers(&[&STAT_ATTACK, &STAT_DPS, &STAT_RANGE, &STAT_ATK_CYCLE]));
    enemies.set_stat_values_1(ModelRc::new(VecModel::from(vec![
        value_cell(&STAT_ATTACK, &stat_ctx),
        value_cell(&STAT_DPS, &stat_ctx),
        value_cell(&STAT_RANGE, &stat_ctx),
        frames_cell(&STAT_ATK_CYCLE, &stat_ctx),
    ])));
    enemies.set_stat_headers_2(headers(&[&STAT_HITPOINTS, &STAT_KNOCKBACKS, &STAT_SPEED, &STAT_CASH_DROP]));
    enemies.set_stat_values_2(ModelRc::new(VecModel::from(vec![
        value_cell(&STAT_HITPOINTS, &stat_ctx),
        value_cell(&STAT_KNOCKBACKS, &stat_ctx),
        value_cell(&STAT_SPEED, &stat_ctx),
        value_cell(&STAT_CASH_DROP, &stat_ctx),
    ])));
    enemies.set_traits(icon_row(images, vault, &traits));
    enemies.set_headline_1(icon_row(images, vault, &headline_1));
    enemies.set_headline_2(icon_row(images, vault, &headline_2));
    enemies.set_body_1(ability_rows(images, vault, &body_1));
    enemies.set_body_2(ability_rows(images, vault, &body_2));
    enemies.set_footer(icon_row(images, vault, &footer));
}

fn refresh_filter(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let enemies = ui.global::<Enemies>();
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let images = &context.images;
    let available = {
        let mut held = state.borrow_mut();

        held.filter.treasure = globals.treasure;

        match (&held.available, &held.enemies) {
            (Some(available), _) => Rc::clone(available),
            (None, Some(enemies)) => {
                let found = Rc::new(present_identities(enemies.iter().map(|enemy| &enemy.stats)));

                held.available = Some(Rc::clone(&found));
                found
            }
            (None, None) => return,
        }
    };
    let filter = state.borrow().filter.clone();
    let view = filter_view(&filter, &available, &globals, &vault, images);

    enemies.set_filter_mode(view.mode);
    enemies.set_filter_mag(view.mag);
    enemies.set_filter_stats(view.stats);
    enemies.set_filter_traits(view.traits);
    enemies.set_filter_attack_types(view.attack_types);
    enemies.set_filter_headline_1(view.headline_1);
    enemies.set_filter_headline_2(view.headline_2);
    enemies.set_filter_body_1(view.body_1);
    enemies.set_filter_body_2(view.body_2);
    enemies.set_filter_footer(view.footer);
}

pub struct FilterView {
    pub mode: i32,
    pub mag: SharedString,
    pub stats: ModelRc<RangeField>,
    pub traits: ModelRc<FilterIcon>,
    pub attack_types: ModelRc<FilterIcon>,
    pub headline_1: ModelRc<FilterIcon>,
    pub headline_2: ModelRc<FilterIcon>,
    pub body_1: ModelRc<FilterLabeled>,
    pub body_2: ModelRc<FilterLabeled>,
    pub footer: ModelRc<FilterIcon>,
}

pub fn filter_view(filter: &EnemyFilterState, available: &HashSet<Identity>, globals: &Globals, vault: &Arc<Vault>, images: &Images) -> FilterView {
    let icon_item = |identity: Identity| {
        let name = get_identity_name(identity, &globals.names);
        let display = get_display_def(identity);

        FilterIcon { icon: icon_of(images, vault, display.icon, "?", &name), active: filter.active_identities.contains(&identity), name: name.into(), id: identity_code(identity) }
    };
    let labeled_item = |identity: Identity| {
        let name = get_identity_name(identity, &globals.names);
        let display = get_display_def(identity);
        let schema = ability_schema(identity);
        let active = filter.active_identities.contains(&identity);
        let ranges: Vec<RangeField> = schema
            .iter()
            .map(|(attr, _)| {
                let range = filter.adv_ranges.get(&identity).and_then(|ranges| ranges.get(attr));

                RangeField {
                    label: (*attr).into(),
                    min: range.map_or(SharedString::default(), |range| range.min.as_str().into()),
                    max: range.map_or(SharedString::default(), |range| range.max.as_str().into()),
                    owner: identity_code(identity),
                }
            })
            .collect();

        FilterLabeled { icon: icon_of(images, vault, display.icon, "?", &name), active, name: name.into(), id: identity_code(identity), expanded: active && !schema.is_empty(), ranges: ModelRc::new(VecModel::from(ranges)) }
    };
    let types: Vec<Identity> = REGISTRY.iter().map(|def| def.identity).filter(|identity| available.contains(identity)).filter(|identity| get_display_def(*identity).group == DisplayGroup::Trait).collect();
    let icons = |items: &[Identity]| ModelRc::new(VecModel::from(items.iter().copied().map(icon_item).collect::<Vec<_>>()));
    let labeled = |items: &[Identity]| ModelRc::new(VecModel::from(items.iter().copied().map(labeled_item).collect::<Vec<_>>()));
    let stats: Vec<RangeField> = STAT_KEYS
        .iter()
        .map(|stat| {
            let range = filter.stat_ranges.get(stat);

            RangeField {
                label: (*stat).into(),
                min: range.map_or(SharedString::default(), |range| range.min.as_str().into()),
                max: range.map_or(SharedString::default(), |range| range.max.as_str().into()),
                owner: -1,
            }
        })
        .collect();

    FilterView {
        mode: if filter.match_mode == MatchMode::And { 0 } else { 1 },
        mag: filter.mag_input.as_str().into(),
        stats: ModelRc::new(VecModel::from(stats)),
        traits: icons(&types),
        attack_types: icons(ATTACK_TYPE_IDENTITIES),
        headline_1: icons(&group_identities(available, DisplayGroup::Headline1)),
        headline_2: icons(&group_identities(available, DisplayGroup::Headline2)),
        body_1: labeled(&group_identities(available, DisplayGroup::Body1)),
        body_2: labeled(&group_identities(available, DisplayGroup::Body2)),
        footer: icons(&group_identities(available, DisplayGroup::Footer)),
    }
}

pub fn jump(ui: &AppWindow, state: &Shared, context: &Rc<Context>, id: u32, magnification: String) {
    select(ui, state, context, id);

    if typable_magnification(&magnification) {
        let mut held = state.borrow_mut();

        held.magnification = parse_magnification(&magnification);
        held.mag_input = magnification;
    }

    refresh(ui, state, context);
}

fn export(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if state.borrow().exporting {
        return;
    }

    let Some(globals) = context.globals() else {
        return;
    };
    let Some(enemy) = state.borrow().selected.and_then(|id| state.borrow().enemy(id)) else {
        return;
    };
    let vault = context.engine.vault();
    let magnification = state.borrow().magnification;
    let data = enemy_statblock::build(Subject { enemy: &enemy, magnification, show_invalid: context.state.settings().show_invalid_enemies(), global: globals.context(&vault), vault: &vault });
    let landing = Rc::clone(state);

    state.borrow_mut().exporting = true;
    detail::export_statblock(ui, context, data, Rc::new(set_export_face), move |_| landing.borrow_mut().exporting = false);
}

