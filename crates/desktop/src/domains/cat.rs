mod abilities;
mod conjure;
mod details;
mod export;
mod filter;
mod list;
mod orbs;
mod talents;
mod ultra;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use iced::alignment::{Horizontal, Vertical};
use iced::futures::channel::mpsc;
use iced::widget::{
    button, column, container, image as iced_image, responsive, row, rule,
    text, text_input, Id, Row, Space,
};
use iced::{Border, Color, Element, Length, Size, Subscription, Task, Theme};
use codex::combat::orb::FIRST_FORM;
use codex::combat::{Entity, Worn};
use tracing::info;

use foundry::common::context::GlobalContext;
use foundry::systems::combat::NameBook;
use foundry::common::formats::SpriteSheet as CoreSpriteSheet;
use foundry::domains::cat::animation as cat_animation;
use foundry::domains::cat::game::stats::{get_final_stats, seeded_level};
use foundry::domains::cat::level;
use foundry::domains::cat::orbs::{self as orb, Allowance};
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::cat::versus;
use foundry::systems::combat::lens::{Lens, Pace};
use foundry::domains::cat::waiter::unitid;
use foundry::domains::cat::CatDataState;
use foundry::domains::sandbox::jailbreak::Grants;
use foundry::systems::combat::registry::{format_stat, Magnification, StatContext, STAT_ATK_CYCLE, STAT_ATTACK, STAT_COOLDOWN, STAT_COST, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS, STAT_RANGE, STAT_RARITY, STAT_SPEED};
use foundry::systems::combat::{Mark, RenderContext};
use foundry::domains::settings::{ScannerConfig, Settings};
use foundry::systems::treasure::Bonus;
use foundry::{Vfs, Vault};

use crate::systems::animation;
use crate::app::state::{AnimState, AppState, CatListState};
use crate::app::theme;
use crate::common::CustomAssets;
use crate::common::SpriteSheet;
use crate::common::header_icon::{self, HeaderIcon};
use crate::editor;
use crate::widget::{grid_frames, grid_header, grid_pair, grid_value, hover_hint, name_box, popup, roster_list, statblock_export, status};

const HEADER_BUTTON_WIDTH: f32 = 65.0;
const HEADER_BUTTON_HEIGHT: f32 = 26.0;
const HEADER_BUTTON_TOP_PADDING: f32 = 5.0;
const EXPORT_BUTTON_RULE_GAP: f32 = 2.0;
const TALENT_HISTORY_CAP: usize = 3;
const EMPTY_CAT_ICON: &str = "uni.png";
const ICON_BOX_WIDTH: f32 = 110.0;
const ICON_BOX_HEIGHT: f32 = 96.0;
const ORB_POPUP: Size = Size::new(310.0, 250.0);
const SLOT_ORB_SIZE: f32 = 44.0;
const ORB_HEADER_SIZE: f32 = 18.0;
const ORB_TOGGLE_HEIGHT: f32 = 18.0;
const ORB_TOGGLE_TEXT: f32 = 11.0;
const ORB_TOGGLE_GAP: f32 = 4.0;
const LOCKED_SLOT: &str = "Opens at Lv60";
const HEADER_SPACING: f32 = 12.0;
const NAME_BOX_WIDTH: f32 = 123.0;
const CARD_WIDTH: f32 = 100.0;
const SLOT_WIDTH: f32 = 48.0;
const SLOT_GAP: f32 = 6.0;
const MORE_BUTTON_HEIGHT: f32 = 24.0;
const MORE_GAP: f32 = 6.0;
const MORE_POPUP: Size = Size::new(260.0, 240.0);

pub(crate) struct CatChanges<'a> {
    pub(crate) images: &'a HashSet<u32>,
    pub(crate) data: &'a HashSet<u32>,
    pub(crate) items: &'a HashSet<u32>,
}

type StatsMemo = RefCell<Option<(u32, Option<Arc<Vec<Entity>>>)>>;

fn animation_preload(state: &mut animation::State, cat: &CatEntry, form: usize, vfs: &Vfs, anim_state: &AnimState) -> Task<Message> {
    let key = cat_animation::set_id(cat, form);

    state.preload(&key, || cat_animation::motions(cat, form, vfs), anim_state).map(Message::Animation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Abilities,
    Talents,
    Details,
    Animation,
}

#[derive(Clone)]
pub enum Message {
    AnimationTick,
    SheetsCheck,
    ScanProgress(u64, usize, usize),
    Loaded(u64, Vec<CatEntry>, Option<u64>, Box<filter::Available>),
    Img015Loaded(u64, usize, Option<CoreSpriteSheet>),
    Img022Loaded(u64, usize, Option<CoreSpriteSheet>),
    SearchChanged(String),
    SelectCat(u32),
    SelectForm(usize),
    JumpToUnit(u32, usize),
    JumpToUnitAtLevel(u32, usize, u16, u16),
    SelectTab(DetailTab),
    LevelInputChanged(String),
    ChangeTalentLevel(u8, u8),
    ToggleTalents(bool),
    ToggleOrigin(bool),
    ToggleParts(bool),
    ToggleWorld(bool),
    ToggleCombat(bool),
    HeaderAction(usize),
    Orbs(orbs::Message),
    OrbPopup(popup::Message),
    OpenMore(Fold),
    MorePopup(popup::Message),
    List(list::Message),
    Filter(filter::Message),
    Abilities(abilities::Message),
    Talents(talents::Message),
    Export(statblock_export::Message),
    Animation(animation::Message),
}

pub(crate) const TABS: [(DetailTab, &str); 4] = [
    (DetailTab::Abilities, "Abilities"),
    (DetailTab::Talents, "Talents"),
    (DetailTab::Details, "Details"),
    (DetailTab::Animation, "Animation"),
];

pub(crate) fn popup_walk(cat: &CatEntry) -> Vec<Message> {
    let groups = cat.talent_data.as_ref().map_or(0, |talent| talent.groups.len());

    (0..groups)
        .filter_map(|group| u8::try_from(group).ok())
        .map(|group| Message::Talents(talents::Message::ToggleGroup(cat.id, group)))
        .chain([Message::ToggleTalents(false), Message::ToggleTalents(true)])
        .chain(TABS.iter().map(|(tab, _)| Message::SelectTab(*tab)))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fold {
    Open,
    Orbs,
    Dials,
}

impl Message {
    pub(crate) fn views_only(&self) -> bool {
        matches!(
            self,
            Self::AnimationTick
                | Self::SheetsCheck
                | Self::Img015Loaded(..)
                | Self::Img022Loaded(..)
                | Self::SelectTab(_)
                | Self::ToggleTalents(_)
                | Self::ToggleOrigin(_)
                | Self::ToggleParts(_)
                | Self::ToggleWorld(_)
                | Self::ToggleCombat(_)
                | Self::Abilities(_)
                | Self::Animation(_)
                | Self::Talents(talents::Message::ToggleGroup(..))
                | Self::Export(_)
        )
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AnimationTick => write!(f, "AnimationTick"),
            Self::SheetsCheck => write!(f, "SheetsCheck"),
            Self::ScanProgress(_, done, total) => write!(f, "ScanProgress({}/{})", done, total),
            Self::Loaded(_, cats, _, _) => write!(f, "Loaded({})", cats.len()),
            Self::Img015Loaded(_, i, _) => write!(f, "Img015Loaded({})", i),
            Self::Img022Loaded(_, i, _) => write!(f, "Img022Loaded({})", i),
            Self::SearchChanged(s) => write!(f, "SearchChanged({})", s),
            Self::SelectCat(id) => write!(f, "SelectCat({})", id),
            Self::SelectForm(i) => write!(f, "SelectForm({})", i),
            Self::JumpToUnit(id, form) => write!(f, "JumpToUnit({}, {})", id, form),
            Self::JumpToUnitAtLevel(id, form, level, plus_level) => {
                write!(f, "JumpToUnitAtLevel({}, {}, {}, {})", id, form, level, plus_level)
            }
            Self::SelectTab(t) => write!(f, "SelectTab({:?})", t),
            Self::LevelInputChanged(s) => write!(f, "LevelInputChanged({})", s),
            Self::ChangeTalentLevel(i, l) => write!(f, "ChangeTalentLevel({}, {})", i, l),
            Self::ToggleTalents(b) => write!(f, "ToggleTalents({})", b),
            Self::ToggleOrigin(b) => write!(f, "ToggleOrigin({})", b),
            Self::ToggleParts(b) => write!(f, "ToggleParts({})", b),
            Self::ToggleWorld(b) => write!(f, "ToggleWorld({})", b),
            Self::ToggleCombat(b) => write!(f, "ToggleCombat({})", b),
            Self::HeaderAction(index) => write!(f, "HeaderAction({})", index),
            Self::Orbs(msg) => write!(f, "Orbs({msg:?})"),
            Self::OrbPopup(_) => write!(f, "OrbPopup"),
            Self::OpenMore(fold) => write!(f, "OpenMore({:?})", fold),
            Self::MorePopup(_) => write!(f, "MorePopup"),
            Self::List(msg) => write!(f, "List({:?})", msg),
            Self::Filter(msg) => write!(f, "Filter({:?})", msg),
            Self::Abilities(msg) => write!(f, "Abilities({:?})", msg),
            Self::Talents(msg) => write!(f, "Talents({:?})", msg),
            Self::Export(msg) => write!(f, "Export({:?})", msg),
            Self::Animation(msg) => write!(f, "Animation({:?})", msg),
        }
    }
}

pub struct State {
    pub data: CatDataState,
    pub selected_cat: Option<u32>,
    pub selected_form: usize,
    pub selected_tab: DetailTab,
    pub search_query: String,

    pub current_level: i32,
    pub level_input: String,
    pub talent_levels: HashMap<u32, HashMap<u8, u8>>,
    pub talent_history: VecDeque<u32>,
    pub talent_level_inputs: HashMap<u8, String>,
    pub worn: HashMap<u32, Vec<Option<u32>>>,

    orbs: orbs::State,
    orb_slot: Option<usize>,
    orb_popup: popup::State,
    orb_spec: popup::Spec,
    orb_locked: bool,
    allowance: Allowance,
    more_popup: popup::State,
    more_spec: popup::Spec,
    more_open: bool,
    more_fold: Fold,

    img015_sheets: Vec<SpriteSheet>,
    img022_sheets: Vec<SpriteSheet>,
    sheet_generation: u64,
    scan_generation: u64,
    custom_assets: CustomAssets,

    dynamic_stats: StatsMemo,
    granted: Option<Grants>,
    header_icon_cache: header_icon::Cache,
    header_icon_dummy: HeaderIcon,

    scan_progress: Option<(usize, usize)>,
    hydrating: bool,
    cached_key: Option<u64>,

    list: list::State,
    filter: filter::State,
    abilities: abilities::State,
    details: details::State,
    talents: talents::State,
    export: statblock_export::State,
    ultra: ultra::State,
    animation: animation::State,
}

impl Default for State {
    fn default() -> Self {
        let header_icon_dummy = HeaderIcon::dummy();

        Self {
            data: CatDataState::default(),
            selected_cat: None,
            selected_form: 0,
            selected_tab: DetailTab::Abilities,
            search_query: String::new(),
            current_level: 1,
            level_input: String::from("1"),
            talent_levels: HashMap::new(),
            talent_history: VecDeque::new(),
            talent_level_inputs: HashMap::new(),
            worn: HashMap::new(),

            orbs: orbs::State::default(),
            orb_slot: None,
            orb_popup: popup::State::default(),
            orb_spec: popup::Spec::new(popup::Kind::CatOrb, ORB_POPUP),
            orb_locked: false,
            allowance: Allowance::default(),
            more_popup: popup::State::default(),
            more_spec: popup::Spec::new(popup::Kind::CatMore, MORE_POPUP),
            more_open: false,
            more_fold: Fold::Dials,

            img015_sheets: Vec::new(),
            img022_sheets: Vec::new(),
            sheet_generation: 0,
            scan_generation: 0,
            custom_assets: CustomAssets::new(),

            dynamic_stats: RefCell::new(None),
            granted: None,
            header_icon_cache: RefCell::new(HashMap::new()),
            header_icon_dummy,

            scan_progress: None,
            hydrating: false,
            cached_key: None,

            list: list::State::default(),
            filter: filter::State::default(),
            abilities: abilities::State::default(),
            details: details::State::default(),
            talents: talents::State::default(),
            export: statblock_export::State::new("Cat"),
            ultra: ultra::State::default(),
            animation: animation::State::with_popup(popup::Kind::CatAnimationExport),
        }
    }
}

pub(crate) struct Inspected<'a> {
    pub(crate) form: usize,
    pub(crate) level: &'a str,
    pub(crate) talents: Option<&'a HashMap<u8, u8>>,
    pub(crate) orbs: Option<&'a Vec<Option<u32>>>,
}

pub(crate) enum Picked {
    Pressed(u32),
    Chosen(u32),
}

impl Message {
    pub(crate) fn picked(&self) -> Option<Picked> {
        match self {
            Self::List(list::Message::Pressed(id)) => Some(Picked::Pressed(*id)),
            Self::List(list::Message::Select(id)) => Some(Picked::Chosen(*id)),
            _ => None,
        }
    }
}

impl State {
    pub(crate) fn inspector(scope: &'static str, banner_form: usize, export: popup::Kind) -> Self {
        Self { list: list::State::scoped(scope, banner_form), animation: animation::State::with_popup(export), details: details::State::inspecting(), ..Self::default() }
    }

    pub(crate) fn orbing(mut self, kind: popup::Kind) -> Self {
        self.orb_spec = popup::Spec::new(kind, ORB_POPUP);
        self
    }

    pub(crate) fn moring(mut self, kind: popup::Kind) -> Self {
        self.more_spec = popup::Spec::new(kind, MORE_POPUP);
        self
    }

    pub(crate) fn lock_orbs(mut self) -> Self {
        self.orb_locked = true;
        self
    }

    pub(crate) fn granting(mut self) -> Self {
        self.granted = Some(Grants::default());
        self
    }

    pub(crate) fn grant(&mut self, id: u32, grants: Grants) {
        if let Some(cat) = self.data.cats.iter_mut().find(|cat| cat.id == id) {
            cat.talent_data = (!grants.talent.groups.is_empty()).then(|| grants.talent.clone());
        }

        self.talent_levels.insert(id, grants.levels.clone());
        self.granted = Some(grants);
    }

    pub(crate) fn set_granting(&mut self, granting: bool) {
        self.granted = granting.then(Grants::default);
    }

    fn talented(&self, form: usize) -> bool {
        self.granted.is_some() || form >= 2
    }

    fn tabbed(&self, form: usize) -> bool {
        self.granted.is_none() && form >= 2
    }

    pub(crate) fn set_banner_form(&mut self, banner_form: usize) {
        self.list.set_variant(banner_form);
    }

    pub(crate) fn set_treasure(&mut self, treasure: Bonus) {
        self.filter.filter_state.treasure = treasure;
        self.list.refresh(&self.data.cats, &self.search_query, &self.filter.filter_state);
    }

    pub(crate) fn adopt_cats(&mut self, cats: &[CatEntry], vault: &Vault) -> Task<Message> {
        self.list.invalidate();
        self.details.clear_icons();
        self.details.clear_combos();
        self.dynamic_stats.replace(None);
        self.header_icon_cache.borrow_mut().clear();
        self.data.cats = cats.to_vec();
        let available = filter::Available::of(&self.data.cats, vault);
        self.list.refresh(&self.data.cats, &self.search_query, &self.filter.filter_state);

        Task::batch([
            self.filter.refresh_available(available).map(Message::Filter),
            self.check_sheets(&vault.vfs),
            self.orbs.load(vault).map(Message::Orbs),
            self.list.take_scroll(),
        ])
    }

    pub(crate) fn inspect(&mut self, id: u32, form: usize, level: &str, talents: &HashMap<u8, u8>, orbs: &[Option<u32>]) {
        self.selected_cat = Some(id);
        self.selected_form = form;
        self.selected_tab = DetailTab::Abilities;
        self.talent_level_inputs.clear();
        self.talent_levels.clear();
        self.talent_levels.insert(id, talents.clone());
        self.worn.clear();
        self.worn.insert(id, orbs.to_vec());
        self.orb_slot = None;
        let caps = self.data.cats.iter().find(|cat| cat.id == id).and_then(|cat| cat.unitbuy.level_caps()).filter(|_| self.granted.is_none());

        self.level_input = level::capped(level, caps);
        self.current_level = level::total(&self.level_input, caps);
        self.animation.clear();
        self.animation.reset_playhead();
    }

    pub(crate) fn inspected(&self) -> Option<Inspected<'_>> {
        let id = self.selected_cat?;

        Some(Inspected { form: self.selected_form, level: &self.level_input, talents: self.talent_levels.get(&id), orbs: self.worn.get(&id) })
    }

    pub(crate) fn restore_list(&mut self, offset: f32, query: &str) -> Task<Message> {
        if self.search_query != query {
            self.search_query = query.to_owned();
        }

        self.list.set_scroll_offset(offset);
        self.list.refresh(&self.data.cats, &self.search_query, &self.filter.filter_state);
        self.list.restore_scroll()
    }

    pub(crate) fn sidebar_view(&self) -> Element<'_, Message> {
        self.view_sidebar(None)
    }

    pub(crate) fn sidebar_view_with<'a>(&'a self, footer: Option<Element<'a, Message>>) -> Element<'a, Message> {
        self.view_sidebar(footer)
    }

    pub(crate) fn detail_view<'a>(&'a self, settings: &'a Settings, app_state: &'a AppState, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        self.view_main_content(settings, app_state, global_ctx)
    }

    fn orb_doors(&self, cat: &CatEntry, vault: &Vault) -> Vec<bool> {
        if self.selected_form < FIRST_FORM || self.granted.is_some() {
            return Vec::new();
        }

        let count = vault.vds.cats.orb_slots(&vault.vfs).get(&cat.id).copied().unwrap_or(0);
        let gates = vault.vds.cats.orb_gates(&vault.vfs);

        orb::open_slots(count, gates.get(&cat.id), self.current_level)
    }

    fn worn_orbs(&self, cat: &CatEntry, vault: &Vault) -> Vec<Worn> {
        let doors = self.orb_doors(cat, vault);
        let held = self.worn.get(&cat.id).map_or(&[][..], Vec::as_slice);

        orb::worn(self.orbs.catalog(), &vault.vds.cats.orb_masks(&vault.vfs), orb::picked(held, &doors))
    }

    fn bare_stats(&self, cat: &CatEntry, global_ctx: GlobalContext<'_>) -> Option<(Entity, Entity)> {
        let dynamic_stats = self.dynamic_stats(&global_ctx.vault.vfs, cat.id);
        let base_stats = dynamic_stats.as_ref().and_then(|forms| forms.get(self.selected_form))?;
        let talented = self.talented(self.selected_form);
        let talent_data = if talented { cat.talent_data.as_ref() } else { None };
        let talent_levels = if talented { self.talent_levels.get(&cat.id) } else { None };

        Some((base_stats.clone(), get_final_stats(base_stats, cat.curve.as_ref(), self.current_level, talent_data, talent_levels, global_ctx.treasure)))
    }

    pub(crate) fn orb_popup_open(&self) -> bool {
        self.orb_slot.is_some() && self.selected_cat.is_some()
    }

    pub fn orb_popup_view<'a>(&'a self, window: Size, names: &'a NameBook) -> Option<Element<'a, Message>> {
        let slot = self.orb_slot?;
        let held = self.worn.get(&self.selected_cat?).and_then(|held| held.get(slot).copied().flatten());

        Some(self.orb_popup.view("Orb", self.orb_spec, window, Message::OrbPopup, move || self.orbs.view(held, &self.allowance, names).map(Message::Orbs), None))
    }

    pub(crate) fn orb_picture<'a, M: 'a>(&self, orb: u32, size: f32) -> Element<'a, M> {
        self.orbs.picture(orb, size)
    }

    pub(crate) fn orb_catalog(&self) -> &[codex::cat::unit::Equipment] {
        self.orbs.catalog()
    }

    fn orb_header<'a>(&'a self, cat: &'a CatEntry, global_ctx: GlobalContext<'_>) -> Option<Element<'a, Message>> {
        let doors = self.orb_doors(cat, global_ctx.vault);

        if doors.is_empty() {
            return None;
        }

        let bare = self.bare_stats(cat, global_ctx).map(|(_, stats)| stats);
        let lens = bare.as_ref().map_or_else(Lens::default, |stats| self.abilities.lens(stats));
        let foe = self.abilities.versus();
        let worn = self.worn_orbs(cat, global_ctx.vault);
        let held = self.worn.get(&cat.id);
        let mut slots = row![].spacing(6);

        for (slot, open) in doors.into_iter().enumerate() {
            let orb = held.and_then(|held| held.get(slot).copied().flatten());

            if !open {
                let label = container(text("Lv60").size(12).style(|theme: &Theme| text::Style { color: Some(theme::weak_text_color(theme)) }))
                    .center_x(Length::Fixed(SLOT_ORB_SIZE))
                    .center_y(Length::Fixed(SLOT_ORB_SIZE));
                let shut = button(label).padding(2).style(theme::neutral_button);

                slots = slots.push(column![hover_hint(shut, LOCKED_SLOT), orb_toggle(None, false)].spacing(ORB_TOGGLE_GAP));
                continue;
            }

            let on = worn.iter().find(|held| held.slot == slot);
            let shine = on.zip(bare.as_ref()).map(|(held, stats)| orb::shine(stats, lens, foe, held)).unwrap_or_default();
            let face: Element<'a, Message> = match orb {
                Some(orb) => self.orbs.shaded(orb, SLOT_ORB_SIZE, !shine.lit, shine.ringed),
                None => container(text("+").size(20)).center_x(Length::Fixed(SLOT_ORB_SIZE)).center_y(Length::Fixed(SLOT_ORB_SIZE)).into(),
            };
            let press = (!self.orb_locked).then_some(Message::HeaderAction(slot));
            let pick = button(face).padding(2).style(theme::neutral_button).on_press_maybe(press);

            slots = slots.push(column![pick, orb_toggle(shine.toggle, shine.lit)].spacing(ORB_TOGGLE_GAP));
        }

        Some(column![theme::bold_text("Orbs").size(ORB_HEADER_SIZE), slots].spacing(6).align_x(Horizontal::Center).into())
    }

    pub(crate) fn seeded(&self, id: u32, banner_form: usize, settings: &Settings) -> Option<(usize, String)> {
        let cat = self.data.cats.iter().find(|cat| cat.id == id)?;

        Some((Self::shown_form(cat, banner_form), seeded_level(cat, settings).1))
    }

    pub(crate) fn shown_form(cat: &CatEntry, banner_form: usize) -> usize {
        let highest = banner_form.min(cat.forms.len() - 1);

        (0..=highest)
            .rev()
            .find(|form| cat.forms[*form] && cat.banner_paths[*form].is_some())
            .or_else(|| (0..=highest).rev().find(|form| cat.forms[*form]))
            .or_else(|| cat.forms.iter().position(|exists| *exists))
            .unwrap_or(0)
    }

    pub(crate) fn img015(&self) -> &[SpriteSheet] {
        &self.img015_sheets
    }

    pub(crate) fn cat(&self, id: u32) -> Option<&CatEntry> {
        self.data.cats.iter().find(|cat| cat.id == id)
    }

    pub(crate) fn reveal(&mut self, id: u32, form: usize, vfs: &Vfs) {
        let Some(cat) = self.data.cats.iter_mut().find(|cat| cat.id == id) else {
            return;
        };
        let Some(present) = cat.forms.get_mut(form) else {
            return;
        };

        *present = true;

        if cat.deploy_icon_paths[form].is_none() {
            cat.deploy_icon_paths[form] = scanner::deploy_icon(vfs, cat, form);
        }
    }

    pub(crate) fn list_scrollable_id() -> Id {
        list::State::scrollable_id()
    }

    pub(crate) fn list_scroll_offset(&self) -> f32 {
        self.list.scroll_offset()
    }

    pub(crate) fn apply_talent_selection(&mut self, cat_id: u32, levels: HashMap<u8, u8>) {
        self.talent_level_inputs.clear();
        self.talent_levels.insert(cat_id, levels);
        self.selected_tab = DetailTab::Abilities;
    }

    pub(crate) fn restore_state(&mut self, state: &CatListState) {
        self.list.set_scroll_offset(state.list_scroll_offset);
        self.selected_cat = state.selected_cat;
        self.selected_form = state.selected_form;
        self.search_query = state.search_query.clone();
        self.talent_levels = state.talent_levels.clone();
        self.talent_history = state.talent_history.clone();
        self.current_level = state.current_level;
        self.level_input = state.level_input.clone();
        self.abilities.set_versus(state.versus);
    }

    pub(crate) fn sync_state(&self, state: &mut CatListState) {
        state.list_scroll_offset = self.list.scroll_offset();
        state.selected_cat = self.selected_cat;
        state.selected_form = self.selected_form;
        state.current_level = self.current_level;
        state.versus = self.abilities.is_versus();

        if state.search_query != self.search_query {
            state.search_query = self.search_query.clone();
        }
        if state.level_input != self.level_input {
            state.level_input = self.level_input.clone();
        }
        if state.talent_history != self.talent_history {
            state.talent_history = self.talent_history.clone();
        }
        if state.talent_levels != self.talent_levels {
            state.talent_levels = self.talent_levels.clone();
        }
    }

    fn dynamic_stats(&self, vfs: &Vfs, id: u32) -> Option<Arc<Vec<Entity>>> {
        if let Some((cached_id, stats)) = self.dynamic_stats.borrow().as_ref()
            && *cached_id == id
        {
            return stats.clone();
        }

        let stats = unitid(vfs, id as i32).map(Arc::new);
        *self.dynamic_stats.borrow_mut() = Some((id, stats.clone()));
        stats
    }

    pub(crate) fn invalidate_assets(&mut self, changed: &CatChanges<'_>, vault: &Vault, config: &ScannerConfig) {
        self.dynamic_stats.replace(None);
        self.animation.invalidate_paths();

        for id in changed.images {
            self.list.forget(*id);
        }

        let mut dropped: Vec<u32> = Vec::new();
        let mut restored: Vec<CatEntry> = Vec::new();

        for id in changed.images.union(changed.data).copied() {
            let Some(index) = self.data.cats.iter().position(|entry| entry.id == id) else {
                restored.extend(scanner::scan_single(id, vault, config));
                continue;
            };

            if changed.data.contains(&id) {
                if let Some(entry) = scanner::scan_single(id, vault, config) {
                    self.data.cats[index] = entry;
                } else {
                    dropped.push(id);
                }

                continue;
            }

            if !scanner::revalidate(&vault.vfs, &mut self.data.cats[index], config) {
                dropped.push(id);
            }
        }

        if !dropped.is_empty() {
            info!(count = dropped.len(), "Dropping cats that no longer have listable data");
            self.data.cats.retain(|entry| !dropped.contains(&entry.id));
        }

        for entry in restored {
            info!(id = entry.id, "Restoring a cat that became listable again");

            let at = self.data.cats.partition_point(|existing| existing.id < entry.id);
            self.data.cats.insert(at, entry);
        }

        for id in changed.items {
            self.details.forget(*id as i32);
        }

        self.details.clear_combos();
        self.filter.refresh_combos(vault);
        self.header_icon_cache.borrow_mut().clear();
        self.list.refresh(&self.data.cats, &self.search_query, &self.filter.filter_state);
        self.reclamp();
    }

    fn reclamp(&mut self) {
        let caps = self.selected_cat.and_then(|id| self.data.cats.iter().find(|cat| cat.id == id)).and_then(|cat| cat.unitbuy.level_caps());

        self.level_input = level::capped(&self.level_input, caps);
        self.current_level = level::total(&self.level_input, caps);
    }

    pub(crate) fn reshare(&mut self, vault: &Vault) {
        scanner::reshare(&mut self.data.cats, vault);
        self.dynamic_stats.replace(None);
    }

    pub(crate) fn reload_selected(&mut self, vault: &Vault, config: &ScannerConfig) {
        let Some(id) = self.selected_cat else { return; };
        let Some(index) = self.data.cats.iter().position(|entry| entry.id == id) else { return; };
        let Some(entry) = scanner::scan_single(id, vault, config) else { return; };

        self.data.cats[index] = entry;
        self.reclamp();
    }

    pub fn set_indexing(&mut self, hydrating: bool) {
        self.scan_progress = Some((0, 0));
        self.hydrating = hydrating;
    }

    pub(crate) fn clear_indexing(&mut self) {
        self.scan_progress = None;
    }

    pub fn icon_stream(&mut self) -> Task<Message> {
        self.list.result_stream().map(Message::List)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.selected_tab != DetailTab::Animation {
            return Subscription::none();
        }

        iced::time::every(self.animation.pace()).map(|_| Message::AnimationTick)
    }

    pub fn start_load(&mut self, settings: &Settings, vault: &Arc<Vault>, active_mod: Option<String>, cached: bool) -> Task<Message> {
        info!(cached, "Triggering cat load");
        let config = settings.scanner_config(active_mod);
        let scan_store = Arc::clone(vault);
        let (tx, rx) = mpsc::unbounded();

        self.scan_generation = self.scan_generation.wrapping_add(1);
        let generation = self.scan_generation;

        thread::spawn(move || {
            if cached && let Some((key, cats)) = scanner::hydrate(&scan_store) {
                let available = Box::new(filter::Available::of(&cats, &scan_store));
                let _ = tx.unbounded_send(Message::Loaded(generation, cats, Some(key), available));
                return;
            }

            let vault = Arc::clone(&scan_store);
            let scan = scanner::load(config, scan_store, |done, total| {
                let _ = tx.unbounded_send(Message::ScanProgress(generation, done, total));
            });

            let payload = scan.key.and_then(|key| scanner::payload(key, &scan.data));
            let available = Box::new(filter::Available::of(&scan.data, &vault));
            let _ = tx.unbounded_send(Message::Loaded(generation, scan.data, scan.key, available));

            if let Some(bytes) = payload {
                scanner::persist(&bytes);
            }
        });

        Task::batch([Task::stream(rx), self.check_sheets(&vault.vfs), self.orbs.load(vault).map(Message::Orbs)])
    }

    pub fn rescan(&mut self, settings: &Settings, vault: &Arc<Vault>, active_mod: Option<String>) -> Task<Message> {
        info!("Rescanning cats");
        self.clear_caches();
        self.start_load(settings, vault, active_mod, false)
    }

    pub(crate) fn cached_key(&self) -> Option<u64> {
        self.cached_key
    }

    pub(crate) fn clear_caches(&mut self) {
        self.dynamic_stats.replace(None);
        self.animation.invalidate_paths();
        self.sheet_generation = self.sheet_generation.wrapping_add(1);
        for sheet in self.img015_sheets.iter_mut().chain(self.img022_sheets.iter_mut()) {
            sheet.mark_stale();
        }
        self.header_icon_cache.borrow_mut().clear();
    }

    fn clamped_selection(&self, cat: &CatEntry) -> (usize, DetailTab) {
        let max_form = cat.forms.iter().rposition(|&exists| exists).unwrap_or(0);
        let current_exists = cat.forms.get(self.selected_form).copied().unwrap_or(false);
        let form = if self.selected_form > max_form || !current_exists {
            max_form
        } else {
            self.selected_form
        };

        let tab = if self.selected_tab == DetailTab::Talents && (!self.tabbed(form) || cat.talent_data.is_none()) {
            DetailTab::Abilities
        } else {
            self.selected_tab
        };

        (form, tab)
    }

    fn push_talent_history(&mut self, id: u32) {
        if let Some(pos) = self.talent_history.iter().position(|&existing| existing == id) {
            self.talent_history.remove(pos);
        }
        self.talent_history.push_back(id);

        while self.talent_history.len() > TALENT_HISTORY_CAP {
            if let Some(evicted) = self.talent_history.pop_front() {
                self.talent_levels.remove(&evicted);
            }
        }
    }

    pub(crate) fn relocalize(&mut self, vfs: &Vfs) -> Task<Message> {
        self.sheet_generation = self.sheet_generation.wrapping_add(1);

        for sheet in self.img015_sheets.iter_mut().chain(self.img022_sheets.iter_mut()) {
            sheet.mark_stale();
        }

        self.check_sheets(vfs)
    }

    fn recut(&self) {
        self.abilities.clear_icons();
        self.talents.clear_icons();
        self.details.clear_icons();
        self.details.clear_combos();
        self.filter.clear_icons();
    }

    fn check_sheets(&mut self, vfs: &Vfs) -> Task<Message> {
        let generation = self.sheet_generation;
        let img015_task = crate::common::img015::ensure_loaded(&mut self.img015_sheets, vfs, false)
            .map(move |(index, sheet)| Message::Img015Loaded(generation, index, sheet));
        let img022_task = crate::common::img022::ensure_loaded(&mut self.img022_sheets, vfs, false)
            .map(move |(index, sheet)| Message::Img022Loaded(generation, index, sheet));

        Task::batch([img015_task, img022_task])
    }

    pub fn update(&mut self, message: Message, settings: &mut Settings, app_state: &mut AppState, global_ctx: GlobalContext<'_>) -> Task<Message> {
        let task = self.update_inner(message, settings, app_state, global_ctx);

        let cat = self.selected_cat.and_then(|id| self.data.cats.iter().find(|c| c.id == id));
        let talent_levels = cat.and_then(|entry| self.talent_levels.get(&entry.id));
        self.ultra.sync(
            ultra::Ctx {
                cat,
                form: self.selected_form,
                talent_levels,
                bump_enabled: settings.cat_data.bump_ultra_60,
            },
            &mut self.current_level,
            &mut self.level_input,
        );

        self.list.refresh(&self.data.cats, &self.search_query, &self.filter.filter_state);

        Task::batch([task, self.list.take_scroll()])
    }

    fn update_inner(&mut self, message: Message, settings: &mut Settings, app_state: &mut AppState, global_ctx: GlobalContext<'_>) -> Task<Message> {
        match message {
            Message::SheetsCheck => self.check_sheets(&global_ctx.vault.vfs),
            Message::Img015Loaded(generation, index, sheet) => {
                if generation == self.sheet_generation
                    && let Some(slot) = self.img015_sheets.get_mut(index)
                {
                    slot.apply(sheet);
                    self.recut();
                }
                Task::none()
            }
            Message::Img022Loaded(generation, index, sheet) => {
                if generation == self.sheet_generation
                    && let Some(slot) = self.img022_sheets.get_mut(index)
                {
                    slot.apply(sheet);
                    self.recut();
                }
                Task::none()
            }
            Message::ScanProgress(generation, done, total) => {
                if generation != self.scan_generation {
                    return Task::none();
                }

                if self.scan_progress.is_none_or(|(prev, _)| done > prev) {
                    self.scan_progress = Some((done, total));
                }
                Task::none()
            }
            Message::Loaded(generation, cats, key, available) => {
                if generation != self.scan_generation {
                    return Task::none();
                }

                info!("Cat load finished with {} entries", cats.len());
                self.cached_key = key;
                self.scan_progress = None;
                self.list.invalidate();
                self.details.clear_icons();
                self.details.clear_combos();
                self.data.cats = cats;
                self.reclamp();
                let filter_task = self.filter.refresh_available(*available).map(Message::Filter);
                let preload_task = match self.selected_cat.and_then(|id| self.data.cats.iter().find(|c| c.id == id)) {
                    Some(cat) => {
                        let (form, tab) = self.clamped_selection(cat);
                        self.selected_form = form;
                        self.selected_tab = tab;
                        if tab != DetailTab::Animation {
                            self.animation.clear();
                        }
                        animation_preload(&mut self.animation, cat, form, &global_ctx.vault.vfs, &app_state.animation)
                    }
                    None => Task::none(),
                };

                Task::batch([filter_task, preload_task])
            }
            Message::AnimationTick => {
                let measuring = match self.selected_cat.and_then(|id| self.data.cats.iter().find(|c| c.id == id)) {
                    Some(cat) => {
                        let vfs = &global_ctx.vault.vfs;
                        let form = self.selected_form;
                        let key = cat_animation::set_id(cat, form);
                        self.animation.sync(&key, || cat_animation::motions(cat, form, vfs), settings, &app_state.animation)
                    }
                    None => Task::none(),
                };
                self.animation.tick();
                measuring.map(Message::Animation)
            }
            Message::SearchChanged(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::SelectCat(id) => {
                if self.selected_cat == Some(id) {
                    return Task::none();
                }

                self.selected_cat = Some(id);
                self.talent_level_inputs.clear();
                self.push_talent_history(id);
                self.animation.reset_playhead();

                info!("Selected cat ID: {}", id);
                match self.data.cats.iter().find(|c| c.id == id) {
                    Some(cat) => {
                        let (level, input) = seeded_level(cat, settings);
                        self.current_level = level;
                        self.level_input = input;
                        let (form, tab) = self.clamped_selection(cat);
                        self.selected_form = form;
                        self.selected_tab = tab;
                        if tab != DetailTab::Animation {
                            self.animation.clear();
                        }
                        animation_preload(&mut self.animation, cat, form, &global_ctx.vault.vfs, &app_state.animation)
                    }
                    None => Task::none(),
                }
            }
            Message::JumpToUnit(id, form) => {
                let Some(cat) = self.data.cats.iter().find(|cat| cat.id == id) else {
                    return Task::none();
                };

                let reachable = cat.forms.get(form).copied().unwrap_or(false);
                let select = self.update(Message::SelectCat(id), settings, app_state, global_ctx);

                if !reachable || self.selected_form == form {
                    return select;
                }

                Task::batch([select, self.update(Message::SelectForm(form), settings, app_state, global_ctx)])
            }
            Message::JumpToUnitAtLevel(id, form, level, plus_level) => {
                let jump = self.update(Message::JumpToUnit(id, form), settings, app_state, global_ctx);
                let input = if plus_level > 0 { format!("{}+{}", level, plus_level) } else { level.to_string() };
                let leveled = self.update(Message::LevelInputChanged(input), settings, app_state, global_ctx);
                Task::batch([jump, leveled])
            }
            Message::SelectForm(form_idx) => {
                self.selected_form = form_idx;

                if self.selected_tab == DetailTab::Talents && form_idx < 2 {
                    self.selected_tab = DetailTab::Abilities;
                }
                if self.selected_tab != DetailTab::Animation {
                    self.animation.clear();
                }
                match self.selected_cat.and_then(|id| self.data.cats.iter().find(|c| c.id == id)) {
                    Some(cat) => animation_preload(&mut self.animation, cat, form_idx, &global_ctx.vault.vfs, &app_state.animation),
                    None => Task::none(),
                }
            }
            Message::SelectTab(tab) => {
                self.selected_tab = tab;

                if tab == DetailTab::Animation {
                    return Task::batch([Task::done(Message::AnimationTick), self.export_scroll_task()]);
                }

                Task::none()
            }
            Message::LevelInputChanged(input) => {
                if !level::typable(&input) {
                    return Task::none();
                }

                let caps = self.selected_cat.and_then(|id| self.data.cats.iter().find(|cat| cat.id == id)).and_then(|cat| cat.unitbuy.level_caps());

                self.level_input = level::capped(&input, caps);
                self.current_level = level::total(&self.level_input, caps);
                Task::none()
            }
            Message::ChangeTalentLevel(index, level) => {
                let Some(cat_id) = self.selected_cat else { return Task::none(); };
                self.talents.set_level(index, level, self.talent_levels.entry(cat_id).or_default(), &mut self.talent_level_inputs);
                Task::none()
            }
            Message::ToggleOrigin(val) => {
                settings.animation.show_origin = val;
                Task::none()
            }
            Message::ToggleParts(val) => {
                settings.animation.show_rig = val;
                Task::none()
            }
            Message::ToggleWorld(val) => {
                settings.animation.show_world = val;
                Task::none()
            }
            Message::ToggleCombat(val) => {
                settings.animation.show_combat = val;
                Task::none()
            }
            Message::HeaderAction(slot) => {
                let open = self.selected_cat.and_then(|id| self.data.cats.iter().find(|cat| cat.id == id)).is_some_and(|cat| self.orb_doors(cat, global_ctx.vault).get(slot).copied().unwrap_or(false));

                if open && !self.orb_locked {
                    self.allowance = self.selected_cat.and_then(|id| self.data.cats.iter().find(|cat| cat.id == id)).map_or_else(Allowance::default, |cat| Allowance::of(cat, global_ctx.vault.vds.cats.orb_masks(&global_ctx.vault.vfs)));
                    self.orb_slot = Some(slot);
                }

                Task::none()
            }
            Message::Orbs(msg) => {
                let held = self.orb_slot.zip(self.selected_cat).and_then(|(slot, id)| self.worn.get(&id)?.get(slot).copied().flatten());

                if let orbs::Outcome::Equip(picked) = self.orbs.update(msg, held, &self.allowance, global_ctx.names)
                    && let Some((slot, id)) = self.orb_slot.zip(self.selected_cat)
                {
                    let count = global_ctx.vault.vds.cats.orb_slots(&global_ctx.vault.vfs).get(&id).copied().unwrap_or(0);
                    let held = self.worn.entry(id).or_default();

                    held.resize(held.len().max(count), None);

                    if let Some(target) = held.get_mut(slot) {
                        *target = picked;
                    }
                }

                Task::none()
            }
            Message::OrbPopup(msg) => {
                if self.orb_popup.update(msg, self.orb_spec) {
                    self.orb_slot = None;
                }

                Task::none()
            }
            Message::OpenMore(fold) => {
                self.more_open = true;
                self.more_fold = fold;

                Task::none()
            }
            Message::MorePopup(msg) => {
                if self.more_popup.update(msg, self.more_spec) {
                    self.more_open = false;
                }

                Task::none()
            }
            Message::ToggleTalents(is_ultra) => {
                let talent_data = self.selected_cat
                    .and_then(|id| self.data.cats.iter().find(|c| c.id == id))
                    .and_then(|cat| cat.talent_data.as_ref());

                if let Some(cat_id) = self.selected_cat
                    && let Some(talent_data) = talent_data {
                    self.talents.toggle(is_ultra, talent_data, self.talent_levels.entry(cat_id).or_default(), &mut self.talent_level_inputs);
                }
                Task::none()
            }
            Message::List(msg) => {
                if let list::Message::Select(id) = msg {
                    return self.update(Message::SelectCat(id), settings, app_state, global_ctx);
                }

                self.list.update(msg);
                Task::none()
            }
            Message::Filter(msg) => {
                self.filter.update(msg);
                Task::none()
            }
            Message::Abilities(msg) => {
                self.abilities.update(msg);
                Task::none()
            }
            Message::Talents(msg) => {
                match msg {
                    talents::Message::LevelChanged(index, level) => {
                        return self.update(Message::ChangeTalentLevel(index, level), settings, app_state, global_ctx);
                    }
                    talents::Message::LevelInputChanged(index, input) => {
                        let talent_data = self.selected_cat
                            .and_then(|id| self.data.cats.iter().find(|c| c.id == id))
                            .and_then(|cat| cat.talent_data.as_ref());
                        let levels = self.selected_cat.map(|id| self.talent_levels.entry(id).or_default());

                        self.talents.set_level_input(index, input, levels, &mut self.talent_level_inputs, talent_data);
                    }
                    talents::Message::ToggleNormal => {
                        return self.update(Message::ToggleTalents(false), settings, app_state, global_ctx);
                    }
                    talents::Message::ToggleUltra => {
                        return self.update(Message::ToggleTalents(true), settings, app_state, global_ctx);
                    }
                    other => self.talents.update(other),
                }
                Task::none()
            }
            Message::Export(msg) => {
                let cat = self.selected_cat.and_then(|id| self.data.cats.iter().find(|c| c.id == id));
                let worn = cat.map_or_else(Vec::new, |cat| self.worn_orbs(cat, global_ctx.vault));
                let ctx = cat.map(|cat| export::Ctx {
                    tab: self.selected_tab,
                    cat,
                    form: self.selected_form,
                    current_level: self.current_level,
                    level_input: &self.level_input,
                    talent_levels: self.talent_levels.get(&cat.id),
                    is_conjure_expanded: self.abilities.is_conjure_expanded(cat.id, settings),
                    sheets: &self.img015_sheets,
                    coins: &self.img022_sheets,
                    global: global_ctx,
                    settings,
                    grants: self.granted.as_ref(),
                    foe: self.abilities.versus().unwrap_or_default(),
                    lens: self.abilities.lens(cat.stats.get(self.selected_form).and_then(Option::as_ref).unwrap_or(&Entity::default())),
                    orbs: worn,
                });

                self.export.update(msg, || ctx.and_then(export::request)).map(Message::Export)
            }
            Message::Animation(msg) => self.animation.update(msg, settings, &mut app_state.animation).map(Message::Animation),
        }
    }

    pub(crate) fn animation_expanded(&self) -> bool {
        self.animation.is_expanded()
    }

    pub(crate) fn selected_motion(&self) -> Option<String> {
        self.animation.selected_label()
    }

    pub fn expanded_animation_view<'a>(&'a self, settings: &'a Settings, app_state: &'a AppState) -> Option<Element<'a, Message>> {
        self.animation.expanded_view(settings, &app_state.animation).map(|view| view.map(Message::Animation))
    }

    pub fn export_popup_open(&self) -> bool {
        self.animation.export_popup_open()
    }

    pub fn export_popup_visible(&self) -> bool {
        self.selected_tab == DetailTab::Animation
    }

    pub(crate) fn set_export_scope(&mut self, scope: String) {
        self.animation.set_export_scope(scope);
    }

    pub fn filter_popup_open(&self) -> bool {
        self.filter.filter_state.is_open
    }

    pub(crate) fn filter_scroll_task<M: 'static>(&self) -> Task<M> {
        self.filter.restore_scroll()
    }

    pub fn filter_popup_view<'a>(&'a self, window: Size, names: &'a NameBook) -> Option<Element<'a, Message>> {
        self.filter
            .filter_state
            .is_open
            .then(|| self.filter.view(&self.img015_sheets, &self.custom_assets, window, names).map(Message::Filter))
    }

    pub(crate) fn export_scroll_task<M: 'static>(&self) -> Task<M> {
        self.animation.export_scroll_task()
    }

    pub fn export_popup_view(&self, window: Size) -> Option<Element<'_, Message>> {
        self.animation.export_popup_view(window).map(|view| view.map(Message::Animation))
    }

    pub fn view<'a>(&'a self, settings: &'a Settings, app_state: &'a AppState, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        let sidebar = self.view_sidebar(None);
        let main_content = self.view_main_content(settings, app_state, global_ctx);

        let base_layout = row![sidebar, main_content]
            .width(Length::Fill)
            .height(Length::Fill);

        base_layout.into()
    }

    fn view_sidebar<'a>(&'a self, footer: Option<Element<'a, Message>>) -> Element<'a, Message> {
        const FILTER_SEARCH_GAP: f32 = 4.0;
        const SEARCH_LIST_GAP: f32 = 8.0;

        let search_input = text_input("Search Cat...", &self.search_query)
            .on_input(Message::SearchChanged)
            .padding(4)
            .size(13)
            .width(Length::Fill)
            .style(theme::rounded_input);

        let filter_button = button(theme::button_label("Filter").size(13))
            .on_press(Message::Filter(filter::Message::Toggle))
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |t: &Theme, status| theme::toggle_button(t, status, self.filter.filter_state.is_active()));

        let cat_list = self.list.view(&self.data.cats, self.selected_cat, self.is_indexing()).map(Message::List);

        let mut sidebar = column![
            filter_button,
            Space::new().height(Length::Fixed(FILTER_SEARCH_GAP)),
            search_input,
            Space::new().height(Length::Fixed(SEARCH_LIST_GAP)),
        ];

        sidebar = sidebar.push(cat_list);

        if let Some(footer) = footer {
            sidebar = sidebar.push(footer);
        }

        container(
            sidebar
                .spacing(0)
                .height(Length::Fill)
        )
            .width(Length::Fixed(roster_list::LIST_WIDTH + 16.0))
            .height(Length::Fill)
            .padding(8)
            .style(theme::list_panel_container)
            .into()
    }

    fn is_indexing(&self) -> bool {
        self.scan_progress.is_some() && self.data.cats.is_empty()
    }

    fn scan_status(&self) -> Option<Element<'_, Message>> {
        if !self.is_indexing() {
            return None;
        }

        let (done, total) = self.scan_progress?;

        if total == 0 {
            return Some(status(if self.hydrating { "Loading Cats..." } else { "Indexing File System..." }, None));
        }

        Some(status("Scanning Cats...", Some(format!("{} / {}", done, total))))
    }

    fn view_main_content<'a>(&'a self, settings: &'a Settings, app_state: &'a AppState, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        if let Some(progress) = self.scan_status() {
            return progress;
        }

        let Some(selected_id) = self.selected_cat else {
            return container(text("Select a Unit").size(24))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        let Some(cat) = self.data.cats.iter().find(|c| c.id == selected_id) else {
            return container(text("No Cat Data"))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        let header = self.view_header(cat, settings, global_ctx);

        let content = match self.selected_tab {
            DetailTab::Abilities => self.view_abilities(cat, settings, global_ctx),
            DetailTab::Talents => self.view_talents(cat, global_ctx),
            DetailTab::Details => self.view_details(cat, global_ctx),
            DetailTab::Animation => editor::target(
                self.animation.view(settings, &app_state.animation).map(Message::Animation),
                editor::Target::CatAnimation,
            ),
        };

        column![
            header,
            Space::new().height(Length::Fixed(8.0)),
            rule::horizontal(1),
            Space::new().height(Length::Fixed(8.0)),
            content
        ]
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding { top: 4.0, right: 16.0, bottom: 16.0, left: 16.0 })
            .into()
    }

    fn view_header<'a>(&'a self, cat: &'a CatEntry, settings: &'a Settings, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        let mut form_row = row![].spacing(4);
        let form_labels = ["Normal", "Evolved", "True", "Ultra"];

        for (i, label) in form_labels.iter().enumerate() {
            let exists = cat.forms.get(i).copied().unwrap_or(false);
            let is_selected = self.selected_form == i;

            let btn = button(theme::centered_text(*label).size(12))
                .width(Length::Fixed(HEADER_BUTTON_WIDTH))
                .height(Length::Fixed(HEADER_BUTTON_HEIGHT))
                .on_press_maybe(exists.then_some(Message::SelectForm(i)))
                .style(move |theme: &Theme, status| button::Style {
                    border: Border { radius: 0.0.into(), ..theme::header_toggle_button(theme, status, is_selected, exists).border },
                    ..theme::header_toggle_button(theme, status, is_selected, exists)
                });

            form_row = form_row.push(btn);
        }

        let form_row = editor::target(form_row, editor::Target::CatForms);

        let mut tab_row = row![].spacing(4);
        let tabs = TABS;

        for (tab_enum, label) in tabs {
            let is_talents = tab_enum == DetailTab::Talents;
            let available = if is_talents {
                self.tabbed(self.selected_form) && cat.talent_data.is_some()
            } else {
                true
            };
            let is_selected = self.selected_tab == tab_enum;

            let btn = button(theme::centered_text(label).size(12))
                .width(Length::Fixed(HEADER_BUTTON_WIDTH))
                .height(Length::Fixed(HEADER_BUTTON_HEIGHT))
                .on_press_maybe(available.then_some(Message::SelectTab(tab_enum)))
                .style(move |theme: &Theme, status| theme::header_toggle_button(theme, status, is_selected, available));

            tab_row = match is_talents {
                true => tab_row.push(editor::target(btn, editor::Target::CatTalents)),
                false => tab_row.push(btn),
            };
        }

        let detail_row = container(responsive(move |size| self.detail_row(cat, settings, global_ctx, size.width))).height(Length::Fixed(ICON_BOX_HEIGHT));

        column![
            Space::new().height(Length::Fixed(HEADER_BUTTON_TOP_PADDING)),
            row![
                form_row,
                container(rule::vertical(1)).height(Length::Fixed(HEADER_BUTTON_HEIGHT - 4.0)),
                tab_row,
            ].spacing(12).align_y(Vertical::Center),
            Space::new().height(Length::Fixed(8.0)),
            rule::horizontal(1),
            Space::new().height(Length::Fixed(8.0)),
            detail_row,
        ].into()
    }

    fn detail_row<'a>(&'a self, cat: &'a CatEntry, settings: &'a Settings, global_ctx: GlobalContext<'a>, width: f32) -> Element<'a, Message> {
        let vfs = &global_ctx.vault.vfs;
        let identity = row![self.view_cat_icon(cat, vfs), self.view_info_box(cat)].spacing(12).align_y(Vertical::Center);
        let (natural, trimmed) = self.header_widths(cat, global_ctx);
        let crushed = width < natural;
        let mut groups: Vec<Element<'a, Message>> = vec![identity.into()];

        if self.selected_tab == DetailTab::Abilities {
            let fold = match width {
                _ if width >= natural => Fold::Open,
                _ if width >= trimmed => Fold::Orbs,
                _ => Fold::Dials,
            };
            let folded = |fold: Fold| Element::from(column![self.export.view().map(Message::Export), self.more_button(MORE_BUTTON_HEIGHT, fold)].spacing(MORE_GAP));

            if fold == Fold::Dials {
                groups.push(Self::divided(15.0, folded(fold)));

                return Row::with_children(groups).spacing(12).align_y(Vertical::Center).into();
            }

            let cards = match cat.stats.get(self.selected_form).and_then(Option::as_ref) {
                Some(stats) => self.abilities.header(stats).map(Message::Abilities),
                None => self.abilities.versus_card().map(Message::Abilities),
            };

            groups.push(Self::divided(15.0, cards));

            if fold == Fold::Orbs {
                groups.push(Self::divided(EXPORT_BUTTON_RULE_GAP, folded(fold)));

                return Row::with_children(groups).spacing(12).align_y(Vertical::Center).into();
            }

            if let Some(orbs) = self.orb_header(cat, global_ctx) {
                groups.push(Self::divided(EXPORT_BUTTON_RULE_GAP, orbs));
            }

            groups.push(Self::divided(EXPORT_BUTTON_RULE_GAP, self.export.view().map(Message::Export)));
        } else if self.selected_tab == DetailTab::Talents
            && let Some(talent_data) = cat.talent_data.as_ref() {
            let header = self.talents.header_view(talent_data, self.talent_levels.get(&cat.id), &cat.talent_costs, &self.img022_sheets, crushed).map(Message::Talents);

            if crushed {
                groups.push(Self::divided(15.0, column![header, self.more_button(talents::COMPACT_BTN_HEIGHT, Fold::Dials)].spacing(talents::COMPACT_GAP).align_x(Horizontal::Center).into()));
            } else {
                groups.push(Self::divided(15.0, header));
                groups.push(Self::divided(EXPORT_BUTTON_RULE_GAP, self.export.view().map(Message::Export)));
            }
        } else if self.selected_tab == DetailTab::Details {
            groups.push(Self::divided(15.0, self.export.view().map(Message::Export)));
        } else if self.selected_tab == DetailTab::Animation {
            groups.push(Self::divided(15.0, animation::debug_toggles(settings, Message::ToggleOrigin, Message::ToggleParts, Message::ToggleWorld, Some(Message::ToggleCombat))));
        }

        Row::with_children(groups).spacing(12).align_y(Vertical::Center).into()
    }

    fn header_widths(&self, cat: &CatEntry, global_ctx: GlobalContext<'_>) -> (f32, f32) {
        let identity = ICON_BOX_WIDTH + HEADER_SPACING + NAME_BOX_WIDTH;
        let divided = |lead: f32, body: f32| HEADER_SPACING + lead + HEADER_SPACING * 3.0 + 1.0 + EXPORT_BUTTON_RULE_GAP + body;

        match self.selected_tab {
            DetailTab::Abilities => {
                let slots = self.orb_doors(cat, global_ctx.vault).len() as f32;
                let orbs = if slots > 0.0 { divided(EXPORT_BUTTON_RULE_GAP, (slots * SLOT_WIDTH + (slots - 1.0) * SLOT_GAP).max(SLOT_WIDTH)) } else { 0.0 };
                let trimmed = identity + divided(15.0, CARD_WIDTH) + divided(EXPORT_BUTTON_RULE_GAP, CARD_WIDTH);

                (trimmed + orbs, trimmed)
            }
            DetailTab::Talents => {
                let natural = identity + divided(15.0, CARD_WIDTH) + divided(EXPORT_BUTTON_RULE_GAP, CARD_WIDTH);

                (natural, natural)
            }
            DetailTab::Details | DetailTab::Animation => (0.0, 0.0),
        }
    }

    fn more_button<'a>(&self, height: f32, fold: Fold) -> Element<'a, Message> {
        button(theme::centered_text("More Actions").size(12))
            .width(Length::Fixed(CARD_WIDTH))
            .height(Length::Fixed(height))
            .padding(0)
            .style(theme::primary_button)
            .on_press(Message::OpenMore(fold))
            .into()
    }

    pub(crate) fn more_popup_open(&self) -> bool {
        self.more_open && self.selected_cat.is_some()
    }

    pub fn more_popup_view<'a>(&'a self, window: Size, global_ctx: GlobalContext<'a>) -> Option<Element<'a, Message>> {
        if !self.more_popup_open() {
            return None;
        }

        let cat = self.data.cats.iter().find(|cat| Some(cat.id) == self.selected_cat)?;

        Some(self.more_popup.view("More Actions", self.more_spec, window, Message::MorePopup, move || self.more_body(cat, global_ctx), None))
    }

    fn more_body<'a>(&'a self, cat: &'a CatEntry, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        let body: Element<'a, Message> = match self.selected_tab {
            DetailTab::Abilities => {
                let cards = match cat.stats.get(self.selected_form).and_then(Option::as_ref) {
                    Some(stats) => self.abilities.header(stats).map(Message::Abilities),
                    None => self.abilities.versus_card().map(Message::Abilities),
                };
                let mut stack = column![].spacing(10).align_x(Horizontal::Center);

                if self.more_fold == Fold::Dials {
                    stack = stack.push(cards);
                }

                if let Some(orbs) = self.orb_header(cat, global_ctx) {
                    if self.more_fold == Fold::Dials {
                        stack = stack.push(rule::horizontal(1));
                    }

                    stack = stack.push(orbs);
                }

                stack.into()
            }
            _ => self.export.view().map(Message::Export),
        };

        container(body).padding(10).center_x(Length::Fill).into()
    }

    fn divided<'a>(lead: f32, body: Element<'a, Message>) -> Element<'a, Message> {
        row![
            Space::new().width(Length::Fixed(lead)),
            container(rule::vertical(1)).height(Length::Fixed(96.0)),
            Space::new().width(Length::Fixed(EXPORT_BUTTON_RULE_GAP)),
            body,
        ]
        .spacing(12)
        .align_y(Vertical::Center)
        .into()
    }

    fn view_cat_icon(&self, cat: &CatEntry, vfs: &Vfs) -> Element<'_, Message> {
        let path = cat.deploy_icon_paths[self.selected_form].as_ref();
        let icon = self.cat_icon_handle(path, vfs);

        editor::target(
            container(iced_image(icon.handle).height(Length::Fixed(ICON_BOX_HEIGHT)))
                .width(Length::Fixed(ICON_BOX_WIDTH))
                .height(Length::Fixed(ICON_BOX_HEIGHT))
                .align_x(Horizontal::Center),
            editor::Target::CatIcon,
        )
    }

    fn cat_icon_handle(&self, path: Option<&PathBuf>, vfs: &Vfs) -> HeaderIcon {
        if let Some(path) = path
            && let Some(icon) = header_icon::load(&self.header_icon_cache, &vfs.source(path))
        {
            return icon;
        }

        vfs.find(EMPTY_CAT_ICON)
            .and_then(|fallback| header_icon::load(&self.header_icon_cache, &vfs.source(&fallback)))
            .unwrap_or_else(|| self.header_icon_dummy.clone())
    }

    fn view_info_box<'a>(&'a self, cat: &'a CatEntry) -> Element<'a, Message> {
        let disp_name = cat.display_name(self.selected_form);

        let id_text = text(format!("ID: {:03}-{}", cat.id, self.selected_form + 1))
            .size(11)
            .style(|theme: &Theme| text::Style { color: Some(Color { a: 0.4, ..theme.palette().text }) });

        let level_row = row![
            text("Level:").size(11).align_y(Vertical::Center),
            text_input("Level", &self.level_input)
                .on_input(Message::LevelInputChanged)
                .size(11)
                .padding(3)
                .width(Length::Fixed(45.0))
                .style(theme::rounded_input)
        ].spacing(6).align_y(Vertical::Center);

        column![
            editor::target(name_box(disp_name, 123.0, 56.0, 145.0), editor::Target::CatExplanation),
            id_text,
            editor::target(level_row, editor::Target::CatLevels),
        ].spacing(0).into()
    }

    fn view_abilities<'a>(&'a self, cat: &'a CatEntry, settings: &'a Settings, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        let dynamic_stats = self.dynamic_stats(&global_ctx.vault.vfs, cat.id);
        let Some(base_stats) = dynamic_stats.as_ref().and_then(|v| v.get(self.selected_form)) else {
            return container(text("Stats data not found")).into();
        };

        let form_allows_talents = self.talented(self.selected_form);
        let talent_data = if form_allows_talents { cat.talent_data.as_ref() } else { None };
        let talent_levels = if form_allows_talents { self.talent_levels.get(&cat.id) } else { None };

        let mut final_stats = get_final_stats(base_stats, cat.curve.as_ref(), self.current_level, talent_data, talent_levels, global_ctx.treasure);

        match self.granted.as_ref() {
            Some(grants) => grants.apply(&mut final_stats),
            None => orb::dress(&mut final_stats, base_stats, self.selected_form, self.abilities.lens(base_stats), &self.worn_orbs(cat, global_ctx.vault)),
        }

        let cat_ctx = RenderContext {
            global: global_ctx,
            base_stats,
            final_stats: &final_stats,
            magnification: Magnification::default(),
            current_level: self.current_level,
            level_curve: cat.curve.as_ref(),
            talent_data,
            talent_levels,
            is_conjure_unit: false,
        };

        let pace = Pace::of(&StatContext::cat(&final_stats, cat.atk_anim_frames[self.selected_form], Some(&cat.unitbuy), global_ctx.treasure));
        let spirit = abilities::SpiritContext {
            cat_id: cat.id,
            global: global_ctx,
            level_curve: cat.curve.as_ref(),
            current_level: self.current_level,
            conjure_unit_id: base_stats.conjure_unit_id,
            foe: self.abilities.versus(),
        };

        editor::target(
            column![
                self.view_stats(cat, &cat_ctx, self.selected_form),
                Space::new().height(Length::Fixed(8.0)),
                self.abilities.view(&cat_ctx, pace, spirit, &self.img015_sheets, &self.custom_assets, settings).map(Message::Abilities)
            ]
                .width(Length::Fill)
                .height(Length::Fill),
            editor::Target::CatAttributes,
        )
    }

    fn view_stats(&self, cat: &CatEntry, ctx: &RenderContext<'_>, form: usize) -> Element<'_, Message> {
        let stat_ctx = StatContext::cat(ctx.final_stats, cat.atk_anim_frames[form], Some(&cat.unitbuy), ctx.global.treasure);
        let cycle = (STAT_ATK_CYCLE.get_value)(&stat_ctx);
        let felt = versus::felt(ctx, &stat_ctx, self.abilities.versus().unwrap_or_default(), self.abilities.lens(ctx.final_stats));
        let shown = |held: &Option<(String, Mark)>, plain: String| held.as_ref().map_or(plain, |(text, _)| text.clone());
        let tone = |held: &Option<(String, Mark)>| held.as_ref().and_then(|(_, mark)| mark.tones()).map(|(upper, _)| Color::from_rgba8(upper[0], upper[1], upper[2], f32::from(upper[3]) / 255.0));

        let atk_str = shown(&felt.attack, format_stat(&STAT_ATTACK, &stat_ctx));
        let dps_str = shown(&felt.dps, format_stat(&STAT_DPS, &stat_ctx));
        let range_str = format_stat(&STAT_RANGE, &stat_ctx);
        let rarity_str = format_stat(&STAT_RARITY, &stat_ctx);
        let hp_str = shown(&felt.hitpoints, format_stat(&STAT_HITPOINTS, &stat_ctx));
        let kb_str = format_stat(&STAT_KNOCKBACKS, &stat_ctx);
        let speed_str = format_stat(&STAT_SPEED, &stat_ctx);
        let cost_str = format_stat(&STAT_COST, &stat_ctx);

        let cd_val = (STAT_COOLDOWN.get_value)(&stat_ctx);

        let first_row = row![
            grid_pair(grid_header(STAT_ATTACK.display_name), grid_value(STAT_ATTACK.display_name, &atk_str), tone(&felt.attack)),
            grid_pair(grid_header(STAT_DPS.display_name), grid_value(STAT_DPS.display_name, &dps_str), tone(&felt.dps)),
            grid_pair(grid_header(STAT_RANGE.display_name), grid_value(STAT_RANGE.display_name, &range_str), None),
            grid_pair(grid_header(STAT_ATK_CYCLE.display_name), grid_frames(STAT_ATK_CYCLE.display_name, cycle), None),
            grid_pair(grid_header(STAT_RARITY.display_name), grid_value(STAT_RARITY.display_name, &rarity_str), None),
        ].spacing(4);

        let second_row = row![
            grid_pair(grid_header(STAT_HITPOINTS.display_name), grid_value(STAT_HITPOINTS.display_name, &hp_str), tone(&felt.hitpoints)),
            grid_pair(grid_header(STAT_KNOCKBACKS.display_name), grid_value(STAT_KNOCKBACKS.display_name, &kb_str), None),
            grid_pair(grid_header(STAT_SPEED.display_name), grid_value(STAT_SPEED.display_name, &speed_str), None),
            grid_pair(grid_header(STAT_COOLDOWN.display_name), grid_frames(STAT_COOLDOWN.display_name, cd_val), None),
            grid_pair(grid_header(STAT_COST.display_name), grid_value(STAT_COST.display_name, &cost_str), None),
        ].spacing(4);

        column![first_row, second_row].spacing(4).into()
    }

    fn view_talents<'a>(&'a self, cat: &'a CatEntry, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        let Some(talent_data) = &cat.talent_data else {
            let empty: Element<'a, Message> =
                container(text("No Talents Available")).width(Length::Fill).height(Length::Fill).into();

            return editor::target(empty, editor::Target::CatTalents);
        };

        let dynamic_stats = self.dynamic_stats(&global_ctx.vault.vfs, cat.id);
        let base_stats = dynamic_stats.as_ref().and_then(|v| v.get(self.selected_form));
        let traits = base_stats.map(|base| {
            get_final_stats(base, cat.curve.as_ref(), self.current_level, Some(talent_data), self.talent_levels.get(&cat.id), global_ctx.treasure)
        });

        let section = self.talents.view(talents::ViewCtx {
            cat_id: cat.id,
            talent_data,
            talent_levels: self.talent_levels.get(&cat.id),
            level_inputs: &self.talent_level_inputs,
            talent_costs: &cat.talent_costs,
            descriptions: &cat.skill_descriptions,
            combos: global_ctx.vault.vds.cats.combo_names(&global_ctx.vault.vfs),
            current_stats: base_stats,
            traits,
            curve: cat.curve.as_ref(),
            unit_level: self.current_level,
            treasure: global_ctx.treasure,
            names: global_ctx.names,
            sheets: &self.img015_sheets,
            img022_sheets: &self.img022_sheets,
            assets: &self.custom_assets,
            vfs: &global_ctx.vault.vfs,
        }).map(Message::Talents);

        editor::target(section, editor::Target::CatTalents)

    }

    fn view_details<'a>(&'a self, cat: &'a CatEntry, global_ctx: GlobalContext<'a>) -> Element<'a, Message> {
        self.details.view(cat, self.selected_form, global_ctx)
    }
}

fn orb_toggle<'a>(toggle: Option<codex::combat::Switch>, lit: bool) -> Element<'a, Message> {
    let label = if toggle.is_some() { orb::TOGGLE_LABEL } else { orb::NO_TOGGLE };

    button(theme::centered_text(label).size(ORB_TOGGLE_TEXT))
        .width(Length::Fixed(SLOT_WIDTH))
        .height(Length::Fixed(ORB_TOGGLE_HEIGHT))
        .padding(0)
        .style(move |t: &Theme, status| theme::toggle_button(t, status, lit && toggle.is_some()))
        .on_press_maybe(toggle.map(|switch| Message::Abilities(abilities::Message::Flip(switch))))
        .into()
}
