use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;
use std::sync::{Arc, OnceLock};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};

use codex::cat::unit::{Talent, TalentGroup};
use codex::combat::{AttrUnit, Entity, Faction, Identity, REGISTRY, get_talent};
use codex::files::{img015, img022};
use foundry::common::io;
use foundry::domains::cat::combo::{self, ComboEffects};
use foundry::domains::cat::filter::evaluation::entity_passes_filter;
use foundry::domains::cat::filter::{ATTACK_TYPE_ICONS, CatFilterState, MatchMode, TalentFilterMode, icons as filter_icons};
use foundry::domains::cat::game::stats::{get_final_stats, seeded_level};
use foundry::domains::cat::game::talents as talent_logic;
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::cat::statblock::{self as cat_statblock, Subject};
use foundry::domains::cat::waiter::unitid;
use foundry::systems::combat::abilities::collect_ability_data;
use foundry::systems::combat::registry::{
    AbilityIcon as RegistryIcon, DisplayGroup, Magnification, STAT_ATK_CYCLE, STAT_ATTACK, STAT_COOLDOWN, STAT_COST, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS,
    STAT_RANGE, STAT_RARITY, STAT_SPEED, StatContext, StatsDef, format_stat, get_display_def, get_fallback_by_icon,
};
use foundry::systems::combat::{AbilityItem, CustomIcon, RenderContext, TalentIdentities, present_identities, talent_identities};
use foundry::Vault;

use crate::catalog::Load;
use crate::context::Globals;
use crate::images::{self, Images};
use crate::pages::detail::{self, IMG015, IMG022};
use crate::pages::Context;
use crate::roster::{Entry, Roster};
use crate::{AbilityIcon, AbilityLine, AppWindow, Cats, ComboCard, ComboEffect, ComboSlot, FilterIcon, FilterLabeled, HeaderBtn, Material, Page, RangeField, Shell, StatCell, TalentRow, Tone};

const FORM_LABELS: [&str; 4] = ["Normal", "Evolved", "True", "Ultra"];
const TAB_LABELS: [&str; 4] = ["Abilities", "Talents", "Details", "Animation"];
const EMPTY_CAT_ICON: &str = "uni.png";
const TALENT_HISTORY_CAP: usize = 3;
const ULTRA_BUMP: i32 = 60;
const MATERIAL_CANVAS: u32 = 128;
const XP_ICON_ID: i32 = 6;
const XP_ICON_HEIGHT: f32 = 32.0;
const COMBO_UNI_SIZE: u32 = 68;
const DESCRIPTION_LINES: usize = 3;
const EVOLVE_TEXT_LINES: usize = 3;
const STAT_KEYS: [&str; 9] = ["Attack", "Dps", "Range", "Atk Cycle (f)", "Hitpoints", "Knockbacks", "Speed", "Cooldown (f)", "Cost"];
const TALENT_MODES: [TalentFilterMode; 3] = [TalentFilterMode::Ignore, TalentFilterMode::Consider, TalentFilterMode::Only];
const MISSING_ICON_ID: usize = 9999;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Abilities,
    Talents,
    Details,
    Animation,
}

impl Tab {
    fn index(self) -> i32 {
        match self {
            Self::Abilities => 0,
            Self::Talents => 1,
            Self::Details => 2,
            Self::Animation => 3,
        }
    }

    fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Talents,
            2 => Self::Details,
            3 => Self::Animation,
            _ => Self::Abilities,
        }
    }
}

struct Available {
    inherent: HashSet<Identity>,
    gainable: TalentIdentities,
    combos: ComboEffects,
}

pub struct State {
    pub cats: Option<Rc<Vec<CatEntry>>>,
    pub selected: Option<u32>,
    pub arriving: Option<(u32, usize)>,
    pub form: usize,
    pub tab: Tab,
    pub level_input: String,
    pub level: i32,
    search: String,
    pub filter: CatFilterState,
    talent_levels: HashMap<u32, HashMap<u8, u8>>,
    talent_inputs: HashMap<u8, String>,
    talent_history: Vec<u32>,
    expanded_groups: HashMap<(u32, u8), bool>,
    conjure_expanded: HashMap<u32, bool>,
    seeded_for: Option<u32>,
    ultra_active: bool,
    saved_level: Option<(i32, String)>,
    stats: HashMap<u32, Option<Arc<Vec<Entity>>>>,
    available: Option<Rc<Available>>,
    pub roster: Rc<Roster>,
    exporting: bool,
    edited: Option<Edited>,
    locked: bool,
}

pub type Edited = Rc<dyn Fn(&AppWindow)>;

fn typable_level(value: &str) -> bool {
    value.chars().all(|glyph| glyph.is_ascii_digit() || glyph == '+')
}

fn parsed_level(input: &str) -> i32 {
    input.split('+').filter_map(|term| term.trim().parse::<i32>().ok()).sum::<i32>().max(1)
}

fn matches(cat: &CatEntry, query: &str) -> bool {
    cat.base_id_str().contains(query) || cat.names.iter().flatten().any(|name| name.to_lowercase().contains(query))
}

fn tooltip(cat: &CatEntry) -> String {
    let mut lines = vec![format!("[ID] {}", cat.base_id_str())];

    for (index, label) in FORM_LABELS.iter().enumerate() {
        if cat.forms[index] {
            lines.push(format!("[{label}] {}", cat.display_name(index)));
        }
    }

    lines.join("\n")
}

pub fn custom_bytes(icon: CustomIcon) -> Option<(&'static str, &'static [u8])> {
    use foundry::common::assets;

    match icon {
        CustomIcon::Multihit => Some(("multihit", assets::MULTIHIT)),
        CustomIcon::Kamikaze => Some(("kamikaze", assets::KAMIKAZE)),
        CustomIcon::BossWave => Some(("boss_wave", assets::BOSS_WAVE)),
        CustomIcon::Dojo => Some(("dojo", assets::DOJO)),
        CustomIcon::StarredAlien => Some(("starred_alien", assets::STARRED_ALIEN)),
        CustomIcon::Burrow => Some(("burrow", assets::BURROW)),
        CustomIcon::Revive => Some(("revive", assets::REVIVE)),
        CustomIcon::Stop => Some(("stop", assets::STOP)),
        CustomIcon::DeathTimer => Some(("death_timer", assets::DEATH_TIMER)),
        CustomIcon::God => Some(("god", assets::GOD)),
        CustomIcon::Unknown => Some(("unknown", assets::UNKNOWN)),
        CustomIcon::None => None,
    }
}

pub fn custom_image(images: &Images, icon: CustomIcon) -> Option<Image> {
    custom_bytes(icon).and_then(|(name, bytes)| images.asset(name, bytes))
}

pub fn icon_of(images: &Images, vault: &Vault, icon: RegistryIcon, fallback: &str, tip: &str) -> AbilityIcon {
    let image = match icon {
        RegistryIcon::Custom(custom) => custom_image(images, custom),
        RegistryIcon::Standard(icon_id) => images.cut(vault, IMG015, icon_id),
        RegistryIcon::None => None,
    };

    match image {
        Some(image) => AbilityIcon { icon: image, has_icon: true, border: Image::default(), has_border: false, fallback: fallback.into(), tip: tip.into() },
        None => AbilityIcon { icon: Image::default(), has_icon: false, border: Image::default(), has_border: false, fallback: fallback.into(), tip: tip.into() },
    }
}

pub fn item_icon(images: &Images, vault: &Vault, item: &AbilityItem) -> AbilityIcon {
    if item.custom_icon != CustomIcon::None
        && let Some(image) = custom_image(images, item.custom_icon)
    {
        return AbilityIcon { icon: image, has_icon: true, border: Image::default(), has_border: false, fallback: SharedString::default(), tip: item.text.as_str().into() };
    }

    if let Some(icon_id) = item.icon_id
        && let Some(image) = images.cut(vault, IMG015, icon_id)
    {
        let border = item.border_id.and_then(|border| images.cut(vault, IMG015, border));

        return AbilityIcon {
            icon: image,
            has_icon: true,
            has_border: border.is_some(),
            border: border.unwrap_or_default(),
            fallback: SharedString::default(),
            tip: item.text.as_str().into(),
        };
    }

    let icon = if item.custom_icon != CustomIcon::None { RegistryIcon::Custom(item.custom_icon) } else { RegistryIcon::Standard(item.icon_id.unwrap_or(MISSING_ICON_ID)) };

    AbilityIcon { icon: Image::default(), has_icon: false, border: Image::default(), has_border: false, fallback: get_fallback_by_icon(icon).into(), tip: item.text.as_str().into() }
}

pub fn icon_row(images: &Images, vault: &Vault, items: &[AbilityItem]) -> ModelRc<AbilityIcon> {
    ModelRc::new(VecModel::from(items.iter().map(|item| item_icon(images, vault, item)).collect::<Vec<_>>()))
}

pub fn ability_rows(images: &Images, vault: &Vault, items: &[AbilityItem]) -> ModelRc<AbilityLine> {
    let rows: Vec<AbilityLine> = items
        .iter()
        .map(|item| AbilityLine {
            icon: item_icon(images, vault, item),
            lines: detail::lines(&item.text),
            conjure: item.icon_id == Some(img015::ICON_CONJURE) && item.custom_icon == CustomIcon::None,
        })
        .collect();

    ModelRc::new(VecModel::from(rows))
}

pub fn headers(defs: &[&StatsDef]) -> ModelRc<StatCell> {
    ModelRc::new(VecModel::from(defs.iter().map(|def| StatCell { header: def.display_name.into(), value: ModelRc::default(), tip: SharedString::default() }).collect::<Vec<_>>()))
}

pub fn text_cell(header: &str, value: &str) -> StatCell {
    StatCell { header: header.into(), value: detail::lines(value), tip: format!("{header}: {value}").into() }
}

pub fn set_edited(state: &Shared, edited: Option<Edited>) {
    state.borrow_mut().edited = edited;
}

pub fn open_filter(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    refresh_filter(ui, state, context);
    ui.global::<Cats>().set_filter_open(true);
}

pub struct Seed<'a> {
    pub cat: &'a CatEntry,
    pub form: usize,
    pub level: &'a str,
    pub talents: &'a HashMap<u8, u8>,
    pub locked: bool,
}

pub fn seed_member(ui: &AppWindow, state: &Shared, context: &Rc<Context>, seed: Seed<'_>) {
    let Seed { cat, form, level: level_input, talents, locked } = seed;

    {
        let mut held = state.borrow_mut();

        held.selected = Some(cat.id);
        held.seeded_for = Some(cat.id);
        held.talent_inputs.clear();
        held.talent_history.retain(|held| *held != cat.id);
        held.talent_history.push(cat.id);
        held.talent_levels.insert(cat.id, talents.clone());
        held.form = form.min(cat.forms.len() - 1);
        held.level = parsed_level(level_input);
        held.level_input = level_input.to_owned();
        held.ultra_active = false;
        held.saved_level = None;
        held.locked = locked;

        if held.tab == Tab::Talents && held.form < 2 {
            held.tab = Tab::Abilities;
        }
    }

    refresh(ui, state, context);
}

pub fn jump(ui: &AppWindow, state: &Shared, context: &Rc<Context>, id: u32, form: usize, level_input: Option<String>) {
    state.borrow_mut().seeded_for = None;
    select(ui, state, context, id, Some(form));

    if let Some(level_input) = level_input.filter(|value| typable_level(value)) {
        {
            let mut held = state.borrow_mut();

            held.level = parsed_level(&level_input);
            held.level_input = level_input;
        }

        refresh(ui, state, context);
    }
}

pub fn value_cell(def: &StatsDef, ctx: &StatContext<'_>) -> StatCell {
    let value = format_stat(def, ctx);

    StatCell { header: def.display_name.into(), value: detail::lines(&value), tip: format!("{}: {value}", def.display_name).into() }
}

pub fn frames_cell(def: &StatsDef, ctx: &StatContext<'_>) -> StatCell {
    let frames = (def.get_value)(ctx);
    let raw = format!("{:.2}s^{frames}f", frames as f32 / 30.0);

    StatCell { header: def.display_name.into(), value: detail::lines(&raw), tip: format!("{}: {raw}", def.display_name).into() }
}

fn ability_schema(icon: RegistryIcon) -> &'static [(&'static str, AttrUnit)] {
    REGISTRY.iter().find(|def| get_display_def(def.identity).icon == icon).map_or(&[], |def| def.schema)
}

fn icon_code(icon: RegistryIcon) -> i32 {
    match icon {
        RegistryIcon::Standard(id) => id as i32,
        RegistryIcon::Custom(custom) => 10_000 + custom as i32,
        RegistryIcon::None => -1,
    }
}

fn icon_from_code(code: i32) -> RegistryIcon {
    if code < 0 {
        return RegistryIcon::None;
    }

    if code >= 10_000 {
        let custom = [
            CustomIcon::None,
            CustomIcon::Multihit,
            CustomIcon::Kamikaze,
            CustomIcon::BossWave,
            CustomIcon::Dojo,
            CustomIcon::StarredAlien,
            CustomIcon::Burrow,
            CustomIcon::Revive,
            CustomIcon::Stop,
            CustomIcon::DeathTimer,
            CustomIcon::God,
            CustomIcon::Unknown,
        ];

        return custom.get((code - 10_000) as usize).map_or(RegistryIcon::None, |custom| RegistryIcon::Custom(*custom));
    }

    RegistryIcon::Standard(code as usize)
}

fn group_icons(available: &Available, target: DisplayGroup, rendered: &mut HashSet<RegistryIcon>) -> Vec<RegistryIcon> {
    let mut found = Vec::new();

    for def in REGISTRY.iter() {
        if !available.inherent.contains(&def.identity) {
            continue;
        }

        let display = get_display_def(def.identity);

        if display.group != target || display.group == DisplayGroup::Trait || ATTACK_TYPE_ICONS.contains(&display.icon) || found.contains(&display.icon) {
            continue;
        }

        found.push(display.icon);
        rendered.insert(display.icon);
    }

    if target == DisplayGroup::Headline2 {
        let kamikaze = RegistryIcon::Custom(CustomIcon::Kamikaze);

        if !found.contains(&kamikaze) {
            found.push(kamikaze);
            rendered.insert(kamikaze);
        }
    }

    found
}

fn gainable(filter: &CatFilterState, available: &Available, identity: Identity) -> bool {
    let normal = filter.talent_mode != TalentFilterMode::Ignore && available.gainable.normal.contains(&identity);
    let ultra = filter.ultra_talent_mode != TalentFilterMode::Ignore && available.gainable.ultra.contains(&identity);

    normal || ultra
}

const NAME_FONT: &[u8] = include_bytes!("../../../website/assets/fonts/FiraSans-Regular.ttf");
const NAME_MAX: f32 = 22.0;
const NAME_WRAP: f32 = 120.0;
const NAME_MARGIN: f32 = 0.94;
const NAME_MIN: f32 = 8.0;
const NAME_STEP: f32 = 0.5;

pub(super) fn name_font() -> Option<&'static FontRef<'static>> {
    static FONT: OnceLock<Option<FontRef<'static>>> = OnceLock::new();

    FONT.get_or_init(|| FontRef::try_from_slice(NAME_FONT).ok()).as_ref()
}

fn glyph_width(font: &FontRef<'static>, size: f32, glyph: char) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    let id = font.glyph_id(glyph);

    if id.0 == 0 { size * 0.9 } else { scaled.h_advance(id) }
}

pub(super) fn text_width(font: &FontRef<'static>, size: f32, text: &str) -> f32 {
    text.chars().map(|glyph| glyph_width(font, size, glyph)).sum()
}

pub(super) fn wrapped(font: &FontRef<'static>, size: f32, text: &str, width: f32) -> (usize, f32) {
    let space = glyph_width(font, size, ' ');
    let mut lines = 1;
    let mut current = 0.0f32;
    let mut widest = 0.0f32;

    for word in text.split_whitespace() {
        let word_width = text_width(font, size, word);
        let joined = if current > 0.0 { current + space + word_width } else { word_width };

        if joined <= width {
            current = joined;
        } else if word_width > width {
            if current > 0.0 {
                lines += 1;
            }

            let full = (word_width / width).floor();

            lines += full as usize;
            current = word_width - full * width;
            widest = widest.max(width);
        } else {
            widest = widest.max(current);
            lines += 1;
            current = word_width;
        }

        widest = widest.max(current.min(width));
    }

    (lines, widest)
}

pub fn name_size(name: &str, wrap_width: f32) -> f32 {
    let Some(font) = name_font() else {
        return NAME_MAX;
    };
    let allowed = if name.split_whitespace().nth(1).is_some() { 2 } else { 1 };
    let room = wrap_width * NAME_MARGIN;
    let mut size = NAME_MAX;

    while size > NAME_MIN {
        let (lines, widest) = wrapped(font, size, name, room);

        if lines <= allowed && widest <= room {
            break;
        }

        size -= NAME_STEP;
    }

    size
}

impl State {
    fn new(images: &Images) -> Self {
        let roster = Roster::new(images, Rc::new(|images: &Images, vault: &Arc<Vault>, path: &Path| (format!("cat-banner:{}", path.display()), images.cat_banner(vault, path))));

        Self {
            cats: None,
            selected: None,
            arriving: None,
            form: 0,
            tab: Tab::Abilities,
            level_input: "1".to_owned(),
            level: 1,
            search: String::new(),
            filter: CatFilterState::default(),
            talent_levels: HashMap::new(),
            talent_inputs: HashMap::new(),
            talent_history: Vec::new(),
            expanded_groups: HashMap::new(),
            conjure_expanded: HashMap::new(),
            seeded_for: None,
            ultra_active: false,
            saved_level: None,
            stats: HashMap::new(),
            available: None,
            roster,
            exporting: false,
            edited: None,
            locked: false,
        }
    }

    pub fn cat(&self, id: u32) -> Option<CatEntry> {
        self.cats.as_ref()?.iter().find(|cat| cat.id == id).cloned()
    }

    fn clamped(&self, cat: &CatEntry) -> (usize, Tab) {
        let max_form = cat.forms.iter().rposition(|&exists| exists).unwrap_or(0);
        let form = if self.form > max_form || !cat.forms.get(self.form).copied().unwrap_or(false) { max_form } else { self.form };
        let tab = if self.tab == Tab::Talents && (form < 2 || cat.talent_data.is_none()) { Tab::Abilities } else { self.tab };

        (form, tab)
    }

    fn seed(&mut self, cat: &CatEntry, settings: &foundry::domains::settings::Settings) {
        if self.seeded_for == Some(cat.id) {
            return;
        }

        self.seeded_for = Some(cat.id);
        self.talent_inputs.clear();
        self.talent_history.retain(|held| *held != cat.id);
        self.talent_history.push(cat.id);

        while self.talent_history.len() > TALENT_HISTORY_CAP {
            let evicted = self.talent_history.remove(0);

            self.talent_levels.remove(&evicted);
        }

        let (level, input) = seeded_level(cat, settings);

        self.level = level;
        self.level_input = input;

        let (form, tab) = self.clamped(cat);

        self.form = form;
        self.tab = tab;
        self.ultra_active = false;
        self.saved_level = None;
        self.sync_ultra(cat, settings);
    }

    pub fn levels_for(&self, id: u32) -> HashMap<u8, u8> {
        self.talent_levels.get(&id).cloned().unwrap_or_default()
    }

    fn sync_ultra(&mut self, cat: &CatEntry, settings: &foundry::domains::settings::Settings) {
        let levels = self.levels_for(cat.id);
        let mut ultra = self.form == 3;

        if self.form >= 2 && !ultra {
            ultra = match &cat.talent_data {
                Some(data) => data.groups.iter().enumerate().any(|(index, group)| group.limit == 1 && levels.get(&(index as u8)).is_some_and(|held| *held > 0)),
                None => levels.iter().any(|(index, held)| *index >= 5 && *held > 0),
            };
        }

        if !settings.cat_data.bump_ultra_60 {
            self.ultra_active = ultra;
            self.saved_level = None;
            return;
        }

        if !self.ultra_active && ultra {
            self.saved_level = Some((self.level, self.level_input.clone()));

            if self.level < ULTRA_BUMP {
                self.level = ULTRA_BUMP;
                self.level_input = ULTRA_BUMP.to_string();
            }
        } else if self.ultra_active
            && !ultra
            && let Some((saved_level, saved_input)) = self.saved_level.take()
        {
            let expected = if saved_level < ULTRA_BUMP { ULTRA_BUMP } else { saved_level };

            if self.level == expected {
                self.level = saved_level;
                self.level_input = saved_input;
            }
        }

        self.ultra_active = ultra;
    }

    fn toggle_talents(&mut self, cat: &CatEntry, ultra: bool, settings: &foundry::domains::settings::Settings) {
        let Some(talent_data) = cat.talent_data.as_ref() else {
            return;
        };
        let levels = self.levels_for(cat.id);
        let availability = toggle_availability(talent_data, Some(&levels));
        let has_enabled = if ultra { availability.has_ultra_enabled } else { availability.has_normal_enabled };
        let held = self.talent_levels.entry(cat.id).or_default();

        for (index, group) in talent_data.groups.iter().enumerate() {
            let targeted = if ultra { group.limit == 1 } else { group.limit != 1 };

            if targeted {
                let index = index as u8;

                held.insert(index, if has_enabled { 0 } else { group.max_level.max(1) });
                self.talent_inputs.remove(&index);
            }
        }

        self.sync_ultra(cat, settings);
    }

    fn stats_of(&mut self, vault: &Vault, id: u32) -> Option<Arc<Vec<Entity>>> {
        self.stats.entry(id).or_insert_with(|| unitid(&vault.vfs, id as i32).map(Arc::new)).clone()
    }

    fn shown(&self) -> Vec<usize> {
        let Some(cats) = &self.cats else {
            return Vec::new();
        };
        let query = self.search.to_lowercase();

        cats.iter()
            .enumerate()
            .filter(|(_, cat)| entity_passes_filter(cat, &self.filter))
            .filter(|(_, cat)| query.is_empty() || matches(cat, &query))
            .map(|(index, _)| index)
            .collect()
    }
}

pub struct ToggleAvailability {
    pub has_normal_enabled: bool,
    pub has_ultra_enabled: bool,
    pub has_ultra_talents: bool,
}

pub fn toggle_availability(talent_data: &Talent, levels: Option<&HashMap<u8, u8>>) -> ToggleAvailability {
    let mut availability = ToggleAvailability { has_normal_enabled: false, has_ultra_enabled: false, has_ultra_talents: false };

    for (index, group) in talent_data.groups.iter().enumerate() {
        let level = levels.and_then(|levels| levels.get(&(index as u8))).copied().unwrap_or(0);

        if group.limit == 1 {
            availability.has_ultra_talents = true;
            availability.has_ultra_enabled |= level > 0;
        } else {
            availability.has_normal_enabled |= level > 0;
        }
    }

    availability
}

pub type Shared = Rc<RefCell<State>>;

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new(&context.images)));
    let cats = window.global::<Cats>();

    cats.set_roster(ModelRc::from(Rc::clone(&state.borrow().roster) as Rc<dyn Model<Data = crate::RosterRow>>));
    cats.set_forms(ModelRc::new(VecModel::from(FORM_LABELS.iter().map(|label| HeaderBtn { label: (*label).into(), available: true }).collect::<Vec<_>>())));
    cats.set_tabs(ModelRc::new(VecModel::from(TAB_LABELS.iter().map(|label| HeaderBtn { label: (*label).into(), available: true }).collect::<Vec<_>>())));

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.catalogs.listen(Rc::new(move |ui: &AppWindow| sync(ui, &held, &ctx)));
    }

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.images.watch(Rc::new(move |ui: &AppWindow, key: &str| {
            if (key.starts_with("trim:") || key.starts_with("box") || key.starts_with("cut:")) && held.borrow().selected.is_some() && (ui.global::<Shell>().get_page() == Page::Cats || ui.global::<crate::Sandbox>().get_unit_open()) {
                refresh(ui, &held, &ctx);
            }
        }));
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_select(move |id| {
        if let Some(ui) = weak.upgrade() {
            select(&ui, &held, &ctx, id.max(0) as u32, None);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_search_changed(move |text| {
        held.borrow_mut().search = text.to_string();

        let shown = held.borrow().shown();

        held.borrow().roster.show(shown.clone());

        if let Some(ui) = weak.upgrade() {
            ui.global::<Cats>().set_roster_count(shown.len() as i32);
        }

        let _ = &ctx;
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_open_filter(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_filter(&ui, &held, &ctx);
            ui.global::<Cats>().set_filter_open(true);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_set_form(move |form| {
        if held.borrow().locked {
            return;
        }

        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(cat) = held.borrow().selected.and_then(|id| held.borrow().cat(id)) else {
            return;
        };

        {
            let mut state = held.borrow_mut();
            let form = form.max(0) as usize;

            if !cat.forms.get(form).copied().unwrap_or(false) {
                return;
            }

            state.form = form;

            if state.tab == Tab::Talents && form < 2 {
                state.tab = Tab::Abilities;
            }

            state.sync_ultra(&cat, &ctx.state.settings());
        }

        ctx.memory.update(|memory| memory.last_cat_form = form.max(0) as usize);
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_set_tab(move |tab| {
        held.borrow_mut().tab = Tab::from_index(tab);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_level_edited(move |value| {
        if held.borrow().locked {
            return;
        }

        if !typable_level(&value) {
            return;
        }

        {
            let mut state = held.borrow_mut();

            state.level = parsed_level(&value);
            state.level_input = value.to_string();
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_conjure_toggled(move || {
        let Some(id) = held.borrow().selected else {
            return;
        };
        let default = ctx.state.settings().cat_data.expand_spirit_details;

        {
            let mut state = held.borrow_mut();
            let next = !state.conjure_expanded.get(&id).copied().unwrap_or(default);

            state.conjure_expanded.insert(id, next);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_export_image(move || {
        if let Some(ui) = weak.upgrade() {
            export(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_toggle_talents(move |ultra| {
        if held.borrow().locked {
            return;
        }

        let Some(cat) = held.borrow().selected.and_then(|id| held.borrow().cat(id)) else {
            return;
        };

        held.borrow_mut().toggle_talents(&cat, ultra, &ctx.state.settings());

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_group_toggled(move |index| {
        let Some(id) = held.borrow().selected else {
            return;
        };

        {
            let mut state = held.borrow_mut();
            let key = (id, index.max(0) as u8);
            let next = !state.expanded_groups.get(&key).copied().unwrap_or(false);

            state.expanded_groups.insert(key, next);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_talent_level(move |index, level| {
        if held.borrow().locked {
            return;
        }

        let Some(cat) = held.borrow().selected.and_then(|id| held.borrow().cat(id)) else {
            return;
        };

        {
            let mut state = held.borrow_mut();
            let index = index.max(0) as u8;

            state.talent_levels.entry(cat.id).or_default().insert(index, level.clamp(0, 255) as u8);
            state.talent_inputs.remove(&index);
            state.sync_ultra(&cat, &ctx.state.settings());
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_talent_input(move |index, input| {
        if held.borrow().locked {
            return;
        }

        let Some(cat) = held.borrow().selected.and_then(|id| held.borrow().cat(id)) else {
            return;
        };

        if !input.chars().all(|glyph| glyph.is_ascii_digit()) {
            return;
        }

        {
            let mut state = held.borrow_mut();
            let index = index.max(0) as u8;
            let max_level = cat.talent_data.as_ref().and_then(|data| data.groups.get(usize::from(index))).map_or(u8::MAX, |group| group.max_level.max(1));
            let trimmed = input.trim();
            let parsed = trimmed.parse::<u8>().ok();
            let levels = state.talent_levels.entry(cat.id).or_default();

            if let Some(parsed) = parsed {
                levels.insert(index, parsed.min(max_level));
            } else if trimmed.is_empty() {
                levels.insert(index, 0);
            }

            let clamped = match parsed {
                Some(parsed) if parsed > max_level => max_level.to_string(),
                _ => input.to_string(),
            };

            state.talent_inputs.insert(index, clamped);
            state.sync_ultra(&cat, &ctx.state.settings());
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    cats.on_jump_cat(move |id, form| {
        if let Some(ui) = weak.upgrade() {
            jump(&ui, &held, &ctx, id.max(0) as u32, form.max(0) as usize, None);
        }
    });

    wire_filter(window, &state, context);

    state
}

fn wire_filter(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let cats = window.global::<Cats>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    fn edited(ui: &AppWindow, held: &Shared, ctx: &Rc<Context>, change: impl FnOnce(&mut CatFilterState)) {
        change(&mut held.borrow_mut().filter);

        let shown = held.borrow().shown();

        held.borrow().roster.show(shown.clone());
        ui.global::<Cats>().set_roster_count(shown.len() as i32);
        ui.global::<Cats>().set_filter_active(held.borrow().filter.is_active());
        refresh_filter(ui, held, ctx);
    }

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_rarity(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                if let Some(slot) = filter.rarities.get_mut(index.max(0) as usize) {
                    *slot = !*slot;
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_form(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| {
                if let Some(slot) = filter.forms.get_mut(index.max(0) as usize) {
                    *slot = !*slot;
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_mode_set(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.match_mode = if index == 0 { MatchMode::And } else { MatchMode::Or });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_talents_set(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.talent_mode = TALENT_MODES.get(index.max(0) as usize).copied().unwrap_or_default());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_ultra_set(move |index| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.ultra_talent_mode = TALENT_MODES.get(index.max(0) as usize).copied().unwrap_or_default());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_level_edited(move |value| {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| filter.level_input = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_stat_min(move |index, value| {
        if let Some(ui) = weak.upgrade()
            && let Some(stat) = STAT_KEYS.get(index.max(0) as usize)
        {
            edited(&ui, &held, &ctx, |filter| filter.stat_ranges.entry(stat).or_default().min = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_stat_max(move |index, value| {
        if let Some(ui) = weak.upgrade()
            && let Some(stat) = STAT_KEYS.get(index.max(0) as usize)
        {
            edited(&ui, &held, &ctx, |filter| filter.stat_ranges.entry(stat).or_default().max = value.to_string());
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_icon(move |code| {
        if let Some(ui) = weak.upgrade() {
            let icon = icon_from_code(code);

            edited(&ui, &held, &ctx, |filter| {
                if !filter.active_icons.remove(&icon) {
                    filter.active_icons.insert(icon);
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_range_min(move |code, index, value| {
        if let Some(ui) = weak.upgrade() {
            let icon = icon_from_code(code);

            if let Some((attr, _)) = ability_schema(icon).get(index.max(0) as usize).copied() {
                edited(&ui, &held, &ctx, |filter| filter.adv_ranges.entry(icon).or_default().entry(attr).or_default().min = value.to_string());
            }
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_range_max(move |code, index, value| {
        if let Some(ui) = weak.upgrade() {
            let icon = icon_from_code(code);

            if let Some((attr, _)) = ability_schema(icon).get(index.max(0) as usize).copied() {
                edited(&ui, &held, &ctx, |filter| filter.adv_ranges.entry(icon).or_default().entry(attr).or_default().max = value.to_string());
            }
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_combo(move |effect| {
        if let Some(ui) = weak.upgrade() {
            let available = held.borrow().available.clone();

            edited(&ui, &held, &ctx, |filter| {
                if !filter.combo_effects.remove(&effect) {
                    filter.combo_effects.insert(effect);
                }

                if let Some(available) = available {
                    filter.combo_units = filter.combo_effects.iter().filter_map(|effect| available.combos.units.get(effect)).flatten().copied().collect();
                }
            });
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    cats.on_filter_clear(move || {
        if let Some(ui) = weak.upgrade() {
            edited(&ui, &held, &ctx, |filter| *filter = CatFilterState::default());
        }
    });
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let cats = ui.global::<Cats>();
    let load = context.catalogs.cats();

    match &load {
        Load::Ready(found) => {
            let fresh = state.borrow().cats.as_ref().is_none_or(|held| !Rc::ptr_eq(held, found));

            if fresh {
                let entries: Vec<Entry> = found.iter().map(|cat| Entry { id: cat.id, banner: cat.image_path.clone(), tooltip: tooltip(cat).into() }).collect();

                {
                    let mut held = state.borrow_mut();

                    held.cats = Some(Rc::clone(found));
                    held.stats.clear();
                    held.available = None;
                    held.seeded_for = None;
                }

                let shown = state.borrow().shown();

                state.borrow().roster.fill(context.engine.vault(), entries, shown.clone());
                cats.set_roster_count(shown.len() as i32);
                cats.set_ready(true);

                let remembered = context.memory.get();
                let wanted = state.borrow().selected.or(remembered.last_cat).filter(|id| found.iter().any(|cat| cat.id == *id));

                match wanted {
                    Some(id) => select(ui, state, context, id, Some(remembered.last_cat_form)),
                    None => {
                        cats.set_selected(-1);
                        cats.set_roster_open(true);
                    }
                }
            }
        }
        Load::Loading(..) | Load::Idle => {
            cats.set_ready(false);
            cats.set_status_title(if load.scanning() { "Scanning Cats...".into() } else { "Loading Cats...".into() });
            cats.set_status_detail(load.detail().unwrap_or_default().into());
        }
    }
}

fn apply(state: &Shared, context: &Rc<Context>, cat: &CatEntry, form: Option<usize>) {
    let mut held = state.borrow_mut();

    held.selected = Some(cat.id);
    held.locked = false;
    held.seed(cat, &context.state.settings());

    if let Some(form) = form.filter(|form| cat.forms.get(*form).copied().unwrap_or(false)) {
        held.form = form;

        if held.tab == Tab::Talents && form < 2 {
            held.tab = Tab::Abilities;
        }
    }
}

pub fn entered(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let shown = state.borrow().selected.and_then(|id| state.borrow().cat(id));

    if shown.is_some() {
        refresh(ui, state, context);
    }
}

pub fn select(ui: &AppWindow, state: &Shared, context: &Rc<Context>, id: u32, form: Option<usize>) {
    let Some(cat) = state.borrow().cat(id) else {
        return;
    };
    let previous = state.borrow().selected.filter(|held| *held != id).and_then(|held| state.borrow().cat(held)).map(|cat| (cat, state.borrow().form));

    apply(state, context, &cat, form);

    let form = state.borrow().form;

    context.memory.update(|memory| {
        memory.last_cat = Some(id);
        memory.last_cat_form = form;
    });
    refresh(ui, state, context);

    let holdable = state.borrow().tab != Tab::Animation && context.images.pending() > 0;

    match previous {
        Some((shown, shown_form)) if holdable => {
            apply(state, context, &shown, Some(shown_form));
            refresh(ui, state, context);
            state.borrow_mut().arriving = Some((id, form));

            let (held, ctx) = (Rc::clone(state), Rc::clone(context));

            context.images.settled(
                ui,
                Instant::now() + images::SETTLE_LIMIT,
                Rc::new(move |ui| {
                    if held.borrow().arriving != Some((id, form)) {
                        return;
                    }

                    held.borrow_mut().arriving = None;

                    let cat = held.borrow().cat(id);

                    if let Some(cat) = cat {
                        apply(&held, &ctx, &cat, Some(form));
                    }

                    refresh(ui, &held, &ctx);
                }),
            );
        }
        _ => state.borrow_mut().arriving = None,
    }
}

pub fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let cats = ui.global::<Cats>();
    let Some(globals) = context.globals() else {
        return;
    };
    let Some(cat) = state.borrow().selected.and_then(|id| state.borrow().cat(id)) else {
        cats.set_selected(-1);
        return;
    };
    let vault = context.engine.vault();
    let images = &context.images;
    let (form, tab, level, level_input) = {
        let held = state.borrow();

        (held.form, held.tab, held.level, held.level_input.clone())
    };

    cats.set_selected(cat.id as i32);
    cats.set_form(form as i32);
    cats.set_tab(tab.index());
    cats.set_level_input(level_input.as_str().into());
    cats.set_forms(ModelRc::new(VecModel::from(FORM_LABELS.iter().enumerate().map(|(index, label)| HeaderBtn { label: (*label).into(), available: cat.forms.get(index).copied().unwrap_or(false) }).collect::<Vec<_>>())));
    cats.set_tabs(ModelRc::new(VecModel::from(
        TAB_LABELS.iter().enumerate().map(|(index, label)| HeaderBtn { label: (*label).into(), available: index != 1 || (form >= 2 && cat.talent_data.is_some()) }).collect::<Vec<_>>(),
    )));

    let name = cat.display_name(form);

    cats.set_name(name.as_str().into());
    cats.set_name_size(name_size(&name, NAME_WRAP));
    cats.set_id_text(format!("ID: {:03}-{}", cat.id, form + 1).into());

    let icon = cat.deploy_icon_paths.get(form).and_then(Option::as_ref).and_then(|path| images.trimmed(&vault, path)).or_else(|| vault.vfs.find(EMPTY_CAT_ICON).and_then(|path| images.trimmed(&vault, &path)));

    cats.set_has_icon(icon.is_some());
    cats.set_icon(icon.unwrap_or_default());

    let stats = state.borrow_mut().stats_of(&vault, cat.id);
    let base = stats.as_ref().and_then(|stats| stats.get(form));

    match tab {
        Tab::Abilities => refresh_abilities(ui, state, context, &globals, &vault, &cat, base, form, level),
        Tab::Talents => refresh_talents(ui, state, &globals, &vault, images, &cat, base, level),
        Tab::Details => refresh_details(ui, &globals, &vault, images, &cat, form, state.borrow().locked),
        Tab::Animation => crate::viewer::show_cat(ui, context, &cat, form),
    }

    if tab != Tab::Animation {
        crate::viewer::hide(context);
    }

    cats.set_has_talents(cat.talent_data.is_some() && form >= 2);
    cats.set_locked(state.borrow().locked);

    let edited = state.borrow().edited.clone();

    if let Some(edited) = edited {
        edited(ui);
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_abilities(ui: &AppWindow, state: &Shared, context: &Rc<Context>, globals: &Rc<Globals>, vault: &Arc<Vault>, cat: &CatEntry, base: Option<&Entity>, form: usize, level: i32) {
    let cats = ui.global::<Cats>();
    let images = &context.images;
    let Some(base) = base else {
        cats.set_stats_missing(true);
        return;
    };

    cats.set_stats_missing(false);

    let allows_talents = form >= 2;
    let talent_data = if allows_talents { cat.talent_data.as_ref() } else { None };
    let levels = allows_talents.then(|| state.borrow().levels_for(cat.id));
    let final_stats = get_final_stats(base, cat.curve.as_ref(), level, talent_data, levels.as_ref(), &globals.treasure);
    let ctx = RenderContext {
        global: globals.context(vault),
        base_stats: base,
        final_stats: &final_stats,
        magnification: Magnification::default(),
        current_level: level,
        level_curve: cat.curve.as_ref(),
        talent_data,
        talent_levels: levels.as_ref(),
        is_conjure_unit: false,
    };
    let (traits, headline_1, headline_2, body_1, body_2, footer) = collect_ability_data(&ctx);
    let stat_ctx = StatContext::cat(&final_stats, cat.atk_anim_frames[form], Some(&cat.unitbuy), &globals.treasure);

    cats.set_stat_headers_1(headers(&[&STAT_ATTACK, &STAT_DPS, &STAT_RANGE, &STAT_ATK_CYCLE, &STAT_RARITY]));
    cats.set_stat_values_1(ModelRc::new(VecModel::from(vec![
        value_cell(&STAT_ATTACK, &stat_ctx),
        value_cell(&STAT_DPS, &stat_ctx),
        value_cell(&STAT_RANGE, &stat_ctx),
        frames_cell(&STAT_ATK_CYCLE, &stat_ctx),
        value_cell(&STAT_RARITY, &stat_ctx),
    ])));
    cats.set_stat_headers_2(headers(&[&STAT_HITPOINTS, &STAT_KNOCKBACKS, &STAT_SPEED, &STAT_COOLDOWN, &STAT_COST]));
    cats.set_stat_values_2(ModelRc::new(VecModel::from(vec![
        value_cell(&STAT_HITPOINTS, &stat_ctx),
        value_cell(&STAT_KNOCKBACKS, &stat_ctx),
        value_cell(&STAT_SPEED, &stat_ctx),
        frames_cell(&STAT_COOLDOWN, &stat_ctx),
        value_cell(&STAT_COST, &stat_ctx),
    ])));
    cats.set_traits(icon_row(images, vault, &traits));
    cats.set_headline_1(icon_row(images, vault, &headline_1));
    cats.set_headline_2(icon_row(images, vault, &headline_2));
    cats.set_body_1(ability_rows(images, vault, &body_1));
    cats.set_body_2(ability_rows(images, vault, &body_2));
    cats.set_footer(icon_row(images, vault, &footer));

    let conjure_open = state.borrow().conjure_expanded.get(&cat.id).copied().unwrap_or(context.state.settings().cat_data.expand_spirit_details);
    let conjures = body_1.iter().chain(body_2.iter()).any(|item| item.icon_id == Some(img015::ICON_CONJURE) && item.custom_icon == CustomIcon::None);

    cats.set_conjure_open(conjure_open && conjures);
    cats.set_has_spirit(false);

    if conjure_open && conjures {
        let spirit = unitid(&vault.vfs, base.conjure_unit_id).and_then(|forms| forms.into_iter().next());

        if let Some(stats) = spirit {
            let final_spirit = get_final_stats(&stats, cat.curve.as_ref(), level, None, None, &globals.treasure);
            let ctx = RenderContext {
                global: globals.context(vault),
                base_stats: &stats,
                final_stats: &final_spirit,
                magnification: Magnification::default(),
                current_level: level,
                level_curve: cat.curve.as_ref(),
                talent_data: None,
                talent_levels: None,
                is_conjure_unit: true,
            };
            let (traits, headline_1, headline_2, body_1, body_2, footer) = collect_ability_data(&ctx);
            cats.set_has_spirit(true);
            cats.set_spirit_icon(icon_of(images, vault, RegistryIcon::Standard(img015::ICON_AREA_ATTACK), "AoE", "Area Attack"));
            cats.set_spirit_damage(detail::lines(&format!("Damage {}\nRange {}", final_spirit.attack_1_damage, final_spirit.standing_range)));
            cats.set_spirit_traits(icon_row(images, vault, &traits));
            cats.set_spirit_headline_1(icon_row(images, vault, &headline_1));
            cats.set_spirit_headline_2(icon_row(images, vault, &headline_2));
            cats.set_spirit_body_1(ability_rows(images, vault, &body_1));
            cats.set_spirit_body_2(ability_rows(images, vault, &body_2));
            cats.set_spirit_footer(icon_row(images, vault, &footer));
        }
    }
}

fn skill_name(images: &Images, vault: &Arc<Vault>, group: &TalentGroup) -> Option<Image> {
    let image_id = if group.name_id > 0 { group.name_id } else { i16::from(group.ability_id) };

    if image_id <= 0 {
        return None;
    }

    let path = vault.vfs.find(&format!("Skill_name_{image_id:03}.png"))?;

    images.trimmed(vault, &path)
}

fn talent_icon(images: &Images, vault: &Vault, group: &TalentGroup) -> AbilityIcon {
    let Some(def) = get_talent(group.ability_id) else {
        return AbilityIcon { icon: Image::default(), has_icon: false, border: Image::default(), has_border: false, fallback: "?".into(), tip: SharedString::default() };
    };
    let display = get_display_def(def.identity);

    icon_of(images, vault, display.icon, display.fallback, "")
}

#[allow(clippy::too_many_arguments)]
fn refresh_talents(ui: &AppWindow, state: &Shared, globals: &Rc<Globals>, vault: &Arc<Vault>, images: &Images, cat: &CatEntry, base: Option<&Entity>, level: i32) {
    let cats = ui.global::<Cats>();
    let Some(talent_data) = cat.talent_data.as_ref() else {
        cats.set_talents(ModelRc::default());
        return;
    };
    let levels = state.borrow().levels_for(cat.id);
    let costs = &cat.talent_costs;
    let descriptions = &cat.skill_descriptions;
    let total = talent_logic::get_total_np_cost(talent_data, &levels, costs);
    let availability = toggle_availability(talent_data, Some(&levels));
    let np_icon = images.cut(vault, IMG022, img022::ICON_NP_COST);

    cats.set_np_total(total.to_string().into());
    cats.set_has_np_icon(np_icon.is_some());
    cats.set_np_icon(np_icon.unwrap_or_default());
    cats.set_normal_label(if availability.has_normal_enabled { "No Talents".into() } else { "All Talents".into() });
    cats.set_ultra_label(if availability.has_ultra_enabled { "No Ultra".into() } else { "All Ultra".into() });
    cats.set_ultra_available(availability.has_ultra_talents);

    let traits = base.map(|base| get_final_stats(base, cat.curve.as_ref(), level, Some(talent_data), Some(&levels), &globals.treasure));
    let expanded = state.borrow().expanded_groups.clone();
    let inputs = state.borrow().talent_inputs.clone();
    let rows: Vec<TalentRow> = talent_data
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let current = levels.get(&(index as u8)).copied().unwrap_or(0);
            let np_cost = talent_logic::get_talent_np_cost(group.cost_id, current, costs);
            let description = descriptions.get(usize::from(group.text_id)).cloned().unwrap_or_else(|| "No skill description found".to_owned());
            let input = inputs.get(&(index as u8)).cloned().unwrap_or_else(|| current.to_string());
            let shown = base.and_then(|stats| {
                let traits = traits.as_ref().unwrap_or(stats);
                let scaling = talent_logic::Scaling { curve: cat.curve.as_ref(), level, treasure: &globals.treasure };

                talent_logic::calculate_linked_display(talent_data, &levels, index, stats, scaling, traits)
            });
            let name_image = skill_name(images, vault, group);
            let fallback_name = globals.names.talent(group.ability_id).map(str::to_owned).unwrap_or_else(|| {
                get_talent(group.ability_id).map_or_else(|| format!("Unknown Skill (ID: {})", group.ability_id), |def| globals.names.ability(def.identity, Faction::Cat).to_owned())
            });

            TalentRow {
                index: index as i32,
                ultra: group.limit == 1,
                icon: talent_icon(images, vault, group),
                has_name_image: name_image.is_some(),
                name_image: name_image.unwrap_or_default(),
                name: fallback_name.into(),
                expanded: expanded.get(&(cat.id, index as u8)).copied().unwrap_or(false),
                description: detail::lines(&description),
                np_cost: np_cost.to_string().into(),
                max_level: i32::from(group.max_level.max(1)),
                level: i32::from(current),
                input: input.into(),
                has_shown: shown.is_some(),
                shown: detail::lines(shown.as_deref().unwrap_or("")),
            }
        })
        .collect();

    cats.set_talents(ModelRc::new(VecModel::from(rows)));
}

fn refresh_details(ui: &AppWindow, globals: &Rc<Globals>, vault: &Arc<Vault>, images: &Images, cat: &CatEntry, form: usize, locked: bool) {
    let cats = ui.global::<Cats>();
    let mut lines = cat
        .description
        .get(form)
        .and_then(Option::as_ref)
        .filter(|lines| lines.iter().any(|line| !line.trim().is_empty()))
        .cloned()
        .unwrap_or_else(|| vec!["No description available".to_owned()]);

    lines.resize(lines.len().max(DESCRIPTION_LINES), String::new());
    cats.set_description(detail::lines(&lines.join("\n")));

    let (materials, xp_cost) = match form {
        2 => (cat.unitbuy.true_form_materials().collect::<Vec<_>>(), cat.unitbuy.true_form_xp_cost),
        3 => (cat.unitbuy.ultra_form_materials().collect::<Vec<_>>(), cat.unitbuy.ultra_form_xp_cost),
        _ => (Vec::new(), 0),
    };
    let text = cat.evolve_text.texts.get(form).and_then(Option::as_ref).filter(|lines| !lines.is_empty()).map(|lines| {
        let mut padded = lines.clone();

        padded.resize(padded.len().max(EVOLVE_TEXT_LINES), String::new());
        padded.join("\n")
    });
    let has_evolve = form >= 2 && (!materials.is_empty() || text.is_some() || xp_cost > 0);

    cats.set_has_evolve(has_evolve);
    cats.set_evolve_text(detail::lines(text.as_deref().unwrap_or("")));

    let material_rows: Vec<Material> = materials
        .iter()
        .map(|material| {
            let icon = io::gatya_item_icon(&vault.vfs, material.item_id).and_then(|path| images.boxed(vault, &path, MATERIAL_CANVAS));
            let name = u32::try_from(material.item_id).ok().and_then(|id| vault.vds.items.name(&vault.vfs, id)).unwrap_or_default();

            Material {
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
                id_label: format!("ID {}", material.item_id).into(),
                quantity: format!("\u{d7}{}", material.quantity).into(),
                tip: name.into(),
            }
        })
        .collect();

    cats.set_materials(ModelRc::new(VecModel::from(material_rows)));
    cats.set_has_xp(xp_cost > 0);
    cats.set_xp_cost(xp_cost.to_string().into());

    let xp = (xp_cost > 0).then(|| io::gatya_item_icon(&vault.vfs, XP_ICON_ID).and_then(|path| images.trimmed(vault, &path))).flatten();

    if let Some(xp) = &xp {
        let (width, height) = Images::size_of(xp);

        cats.set_xp_width(if height > 0 { XP_ICON_HEIGHT * width as f32 / height as f32 } else { XP_ICON_HEIGHT });
    }

    cats.set_has_xp_icon(xp.is_some());
    cats.set_xp_icon(xp.unwrap_or_default());

    let combos = combo::combos(globals.context(vault), cat.id, form);
    let cards: Vec<ComboCard> = combos
        .iter()
        .map(|held| ComboCard {
            name: held.name.as_str().into(),
            effect: held.effect.trim().into(),
            restriction: held.restriction.clone().unwrap_or_default().into(),
            slots: ModelRc::new(VecModel::from(
                held.members
                    .iter()
                    .map(|member| {
                        let icon = member.icon.as_ref().and_then(|source| images.boxed(vault, &source.path, COMBO_UNI_SIZE));
                        let target = member.id.filter(|id| *id != cat.id && !locked);

                        ComboSlot {
                            has_icon: icon.is_some(),
                            icon: icon.unwrap_or_default(),
                            label: member.label().unwrap_or_default().into(),
                            tip: member.name.clone().or_else(|| member.label()).unwrap_or_default().into(),
                            cat_id: target.map_or(-1, |id| id as i32),
                            form: member.form as i32,
                            jumpable: target.is_some(),
                        }
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();

    cats.set_combos(ModelRc::new(VecModel::from(cards)));
}

fn refresh_filter(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let cats = ui.global::<Cats>();
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let images = &context.images;
    let available = {
        let held = state.borrow();

        match (&held.available, &held.cats) {
            (Some(available), _) => Some(Rc::clone(available)),
            (None, Some(cats)) => Some(Rc::new(Available {
                inherent: present_identities(cats.iter().flat_map(|cat| cat.stats.iter().flatten())),
                gainable: talent_identities(cats.iter().filter_map(|cat| cat.talent_data.as_ref())),
                combos: combo::effects(&vault),
            })),
            (None, None) => None,
        }
    };
    let Some(available) = available else {
        return;
    };

    {
        let mut held = state.borrow_mut();

        held.available = Some(Rc::clone(&available));
        held.filter.treasure = globals.treasure;
    }

    let filter = state.borrow().filter.clone();
    let icon_item = |icon: RegistryIcon| {
        let name = filter_icons::get_icon_name(&icon, &globals.names);

        FilterIcon { icon: icon_of(images, &vault, icon, "?", &name), active: filter.active_icons.contains(&icon), name: name.into(), id: icon_code(icon) }
    };
    let labeled_item = |icon: RegistryIcon| {
        let name = filter_icons::get_icon_name(&icon, &globals.names);
        let schema = ability_schema(icon);
        let active = filter.active_icons.contains(&icon);
        let ranges: Vec<RangeField> = schema
            .iter()
            .map(|(attr, _)| {
                let range = filter.adv_ranges.get(&icon).and_then(|ranges| ranges.get(attr));

                RangeField {
                    label: (*attr).into(),
                    min: range.map_or(SharedString::default(), |range| range.min.as_str().into()),
                    max: range.map_or(SharedString::default(), |range| range.max.as_str().into()),
                    owner: icon_code(icon),
                }
            })
            .collect();

        FilterLabeled { icon: icon_of(images, &vault, icon, "?", &name), active, name: name.into(), id: icon_code(icon), expanded: active && !schema.is_empty(), ranges: ModelRc::new(VecModel::from(ranges)) }
    };
    let traits: Vec<FilterIcon> = REGISTRY
        .iter()
        .filter(|def| available.inherent.contains(&def.identity))
        .map(|def| get_display_def(def.identity))
        .filter(|display| display.group == DisplayGroup::Trait)
        .map(|display| icon_item(display.icon))
        .collect();
    let mut rendered: HashSet<RegistryIcon> = HashSet::new();
    let headline_1 = group_icons(&available, DisplayGroup::Headline1, &mut rendered);
    let headline_2 = group_icons(&available, DisplayGroup::Headline2, &mut rendered);
    let body_1 = group_icons(&available, DisplayGroup::Body1, &mut rendered);
    let body_2 = group_icons(&available, DisplayGroup::Body2, &mut rendered);
    let footer = group_icons(&available, DisplayGroup::Footer, &mut rendered);
    let check_talents = filter.talent_mode != TalentFilterMode::Ignore || filter.ultra_talent_mode != TalentFilterMode::Ignore;
    let mut talents: Vec<RegistryIcon> = Vec::new();

    if check_talents {
        for def in REGISTRY.iter() {
            if !gainable(&filter, &available, def.identity) {
                continue;
            }

            let display = get_display_def(def.identity);

            if display.group == DisplayGroup::Trait || rendered.contains(&display.icon) || ATTACK_TYPE_ICONS.contains(&display.icon) || talents.contains(&display.icon) {
                continue;
            }

            talents.push(display.icon);
        }
    }

    let icons = |items: &[RegistryIcon]| ModelRc::new(VecModel::from(items.iter().copied().map(icon_item).collect::<Vec<_>>()));
    let labeled = |items: &[RegistryIcon]| ModelRc::new(VecModel::from(items.iter().copied().map(labeled_item).collect::<Vec<_>>()));

    cats.set_filter_rarities(ModelRc::new(VecModel::from(filter.rarities.to_vec())));
    cats.set_filter_forms(ModelRc::new(VecModel::from(filter.forms.to_vec())));
    cats.set_filter_mode(if filter.match_mode == MatchMode::And { 0 } else { 1 });
    cats.set_filter_talents(TALENT_MODES.iter().position(|mode| *mode == filter.talent_mode).unwrap_or(0) as i32);
    cats.set_filter_ultra(TALENT_MODES.iter().position(|mode| *mode == filter.ultra_talent_mode).unwrap_or(0) as i32);
    cats.set_filter_level(filter.level_input.as_str().into());
    cats.set_filter_stats(ModelRc::new(VecModel::from(
        STAT_KEYS
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
            .collect::<Vec<_>>(),
    )));
    cats.set_filter_traits(ModelRc::new(VecModel::from(traits)));
    cats.set_filter_attack_types(icons(ATTACK_TYPE_ICONS));
    cats.set_filter_headline_1(icons(&headline_1));
    cats.set_filter_headline_2(icons(&headline_2));
    cats.set_filter_body_1(labeled(&body_1));
    cats.set_filter_body_2(labeled(&body_2));
    cats.set_filter_footer(icons(&footer));
    cats.set_filter_talent_icons(icons(&talents));

    let groups: Vec<ModelRc<ComboEffect>> = available
        .combos
        .groups
        .iter()
        .map(|group| {
            ModelRc::new(VecModel::from(
                group
                    .iter()
                    .copied()
                    .filter_map(|effect| available.combos.labels.get(&effect).map(|label| ComboEffect { label: label.as_str().into(), active: filter.combo_effects.contains(&effect), id: effect }))
                    .collect::<Vec<_>>(),
            ))
        })
        .collect();

    cats.set_filter_combos(ModelRc::new(VecModel::from(groups)));
}

fn export(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if state.borrow().exporting {
        return;
    }

    let Some(globals) = context.globals() else {
        return;
    };
    let Some(cat) = state.borrow().selected.and_then(|id| state.borrow().cat(id)) else {
        return;
    };
    let vault = context.engine.vault();
    let (form, level, level_input, talent_levels, conjure) = {
        let held = state.borrow();
        let talents = (held.form >= 2).then(|| held.levels_for(cat.id));

        (held.form, held.level, held.level_input.clone(), talents, held.conjure_expanded.get(&cat.id).copied().unwrap_or(context.state.settings().cat_data.expand_spirit_details))
    };
    let Some(data) = cat_statblock::build(Subject {
        cat: &cat,
        form,
        current_level: level,
        level_input: &level_input,
        talent_levels: talent_levels.as_ref(),
        is_conjure_expanded: conjure,
        global: globals.context(&vault),
    }) else {
        context.toast(ui, "The statblock could not be built", true);
        return;
    };
    let landing = Rc::clone(state);

    state.borrow_mut().exporting = true;
    detail::export_statblock(ui, context, data, Rc::new(set_export_face), move |_| landing.borrow_mut().exporting = false);
}

pub fn set_export_face(ui: &AppWindow, label: &str, tone: Tone) {
    let cats = ui.global::<Cats>();

    cats.set_export_label(label.into());
    cats.set_export_tone(tone);
}
