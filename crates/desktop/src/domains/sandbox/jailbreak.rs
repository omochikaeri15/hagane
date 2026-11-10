use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced::alignment::{Horizontal, Vertical};
use iced::widget::image::Handle;
use iced::widget::{button, column, container, image as iced_image, mouse_area, row, scrollable, sensor, slider, stack, text, text_input, Column, Row, Space};
use iced::{mouse, Element, Length, Padding, Point, Size, Subscription, Task, Theme};

use emulator::runtime::{RoguelikePicks, RosterPick, SetupUnit};
use foundry::common::context::GlobalContext;
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::sandbox::config::Config;
use foundry::common::formats;
use foundry::domains::sandbox::jailbreak::{self as rules, Board, Bonus, Run, Runs, Tally};
use foundry::domains::sandbox::{Arrangement, Cell, Lineup, Member, Placed, LINEUP_SLOTS};
use foundry::domains::settings::Settings;
use foundry::systems::combat::registry::AbilityIcon;
use foundry::systems::combat::NameBook;
use foundry::Source;

use crate::app::state::AppState;
use crate::app::theme;
use crate::common::ability_icon;
use crate::common::feedback::{Slot as Confirm, CONFIRM_SHORT_LABEL};
use crate::common::header_icon;
use crate::domains::cat::{self, Picked};
use crate::editor;
use crate::widget::{fallback_icon, hover_hint, list_row, popup, smooth_scroll};

use super::combos;
use super::lineup::{cell_badge, cell_frame, cell_outline, deck_card, Landing, Metrics, BADGE_SIZE, CARD_GAP, DOUBLE_CLICK, GHOST_ALPHA, LEFT_WIDTH, LEVEL_SIZE, NAME_SIZE, RIGHT_WIDTH, RIPE, SLACK, TOP_PAD};

const GLOBAL_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SandboxJailbreakGlobal, Size::new(700.0, 480.0));
const CAT_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SandboxJailbreakCat, Size::new(700.0, 480.0));
const UNIT_POPUP: popup::Spec = popup::Spec::new(popup::Kind::SandboxJailbreakUnit, Size::new(760.0, 540.0));
const LIST_SCOPE: &str = "sandbox-jailbreak-banner-list";
const ACTION_WIDTH: f32 = 120.0;
const ACTION_HEIGHT: f32 = 26.0;
const ACTION_SIZE: f32 = 12.0;
const PANE_PADDING: f32 = 16.0;
const LIST_SPACING: f32 = 8.0;
const GROUP_SPACING: f32 = 14.0;
const TEXT_SIZE: f32 = 13.0;
const NOTE_SIZE: f32 = 12.0;
const TITLE_SIZE: f32 = 14.0;
const ICON_SIZE: f32 = 36.0;
const SLIDER_WIDTH: f32 = 110.0;
const COUNT_WIDTH: f32 = 30.0;
const TALLY_SIZE: f32 = 11.0;
const TALLY_PADDING: [f32; 2] = [4.0, 10.0];
const TALLY_CLEARANCE: f32 = 36.0;
const TALLY_MARGIN: f32 = 6.0;
const FOOTER_GAP: f32 = 8.0;
const TILE_PADDING: [f32; 2] = [4.0, 6.0];
const RAIL_WIDTH: f32 = 8.0;
const RAIL_MARGIN: f32 = 2.0;
const HOLD: Duration = Duration::from_millis(700);
const NO_BONUSES: &str = "No bonus matches";

pub type Key = (i32, i32, u64, usize);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cargo {
    Fresh(u32),
    Held(Cell),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Drag {
    #[default]
    Idle,
    Pressed {
        cargo: Cargo,
        since: Instant,
        from: Option<Point>,
    },
    Moving {
        cargo: Cargo,
        lifted: Option<Cell>,
        at: Point,
        landing: Landing,
    },
}

#[derive(Clone, Debug)]
pub enum Message {
    Cat(cat::Message),
    Combos(combos::Message),
    OpenGlobal,
    OpenCat,
    GlobalPopup(popup::Message),
    CatPopup(popup::Message),
    UnitPopup(popup::Message),
    Measured(Size),
    PressCell(Cell),
    DragMove(Point),
    DragEnd,
    DragCancel,
    Held,
    Raise(i32, i32),
    GlobalQuery(String),
    CatQuery(String),
    Create,
    Delete,
    DeleteExpired,
    Load(usize),
    Rename(String),
    Search(String),
}

fn abandoned(event: iced::Event, _status: iced::event::Status, _window: iced::window::Id) -> Option<Message> {
    matches!(event, iced::Event::Mouse(mouse::Event::CursorLeft) | iced::Event::Window(iced::window::Event::Unfocused)).then_some(Message::DragCancel)
}

struct Saved {
    index: usize,
    name: String,
    count: String,
}

struct Offer {
    index: usize,
    level: i32,
    count: String,
    label: String,
}

struct Group {
    title: String,
    offers: Vec<Offer>,
}

pub struct State {
    inspector: cat::State,
    combos: combos::State,
    global_popup: popup::State,
    cat_popup: popup::State,
    unit_popup: popup::State,
    global_open: bool,
    cat_open: bool,
    viewing: Option<u32>,
    shown: Option<Member>,
    key: Option<Key>,
    board: Option<Board>,
    stage: i32,
    tally: Option<Tally>,
    run: Run,
    lineup: Lineup,
    forms: HashMap<u32, usize>,
    adopted: Option<Vec<(u32, usize)>>,
    drag: Drag,
    plan: Option<Arrangement>,
    area: Size,
    metrics: Metrics,
    clicked: Option<(Cell, Instant)>,
    holding: Confirm<Cell>,
    icons: HashMap<(u32, usize), Handle>,
    decoded: header_icon::Cache,
    bonus_icons: ability_icon::Cache,
    base_icons: HashMap<usize, Handle>,
    global_query: String,
    cat_query: String,
    global: Vec<Group>,
    cat: Vec<Group>,
    deleting: Confirm<()>,
    search: String,
    saved: Vec<Saved>,
    selected: usize,
    named: String,
    stale: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            inspector: cat::State::inspector(LIST_SCOPE, 0, popup::Kind::JailbreakAnimationExport).granting(),
            combos: combos::State::default(),
            global_popup: popup::State::default(),
            cat_popup: popup::State::default(),
            unit_popup: popup::State::default(),
            global_open: false,
            cat_open: false,
            viewing: None,
            shown: None,
            key: None,
            board: None,
            stage: 0,
            tally: None,
            run: Run::default(),
            lineup: Lineup::default(),
            forms: HashMap::new(),
            adopted: None,
            drag: Drag::Idle,
            plan: None,
            area: Size::ZERO,
            metrics: Metrics::FULL,
            clicked: None,
            holding: Confirm::default(),
            icons: HashMap::new(),
            decoded: header_icon::Cache::default(),
            bonus_icons: ability_icon::Cache::default(),
            base_icons: HashMap::new(),
            global_query: String::new(),
            cat_query: String::new(),
            global: Vec::new(),
            cat: Vec::new(),
            deleting: Confirm::default(),
            search: String::new(),
            saved: Vec::new(),
            selected: 0,
            named: String::new(),
            stale: true,
        }
    }
}

fn matches(label: &str, query: &str) -> bool {
    query.is_empty() || label.to_lowercase().contains(query)
}

impl State {
    pub fn key(&self) -> Option<Key> {
        self.key
    }

    pub fn ready(&self) -> bool {
        self.board.is_some()
    }

    pub fn global_open(&self) -> bool {
        self.global_open && self.board.is_some()
    }

    pub fn cat_open(&self) -> bool {
        self.cat_open && self.board.is_some()
    }

    pub fn unit_open(&self) -> bool {
        self.board.is_some() && self.viewing.is_some()
    }

    pub fn export_open(&self) -> bool {
        self.unit_open() && self.inspector.export_popup_open()
    }

    pub fn filter_open(&self) -> bool {
        self.board.is_some() && self.inspector.filter_popup_open()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let inspecting = if self.unit_open() { self.inspector.subscription().map(Message::Cat) } else { Subscription::none() };
        let dragging = if self.drag == Drag::Idle { Subscription::none() } else { iced::event::listen_with(abandoned) };

        Subscription::batch([inspecting, dragging])
    }

    pub fn icon_stream(&mut self) -> Task<Message> {
        self.inspector.icon_stream().map(Message::Cat)
    }

    pub fn set_board(&mut self, key: Option<Key>, board: Option<Board>, config: &Config) {
        self.key = key;
        self.board = board;
        self.global_open = false;
        self.cat_open = false;
        self.combos.invalidate();
        self.combos.focus(None);
        self.viewing = None;
        self.drag = Drag::Idle;
        self.plan = None;
        self.adopted = None;
        self.icons.clear();
        self.base_icons.clear();
        self.decoded.borrow_mut().clear();
        self.deleting.clear();
        self.stale = true;
        self.settle(config);
    }

    pub fn set_stage(&mut self, stage: i32, config: &Config) {
        if self.stage == stage {
            return;
        }

        self.stage = stage;
        self.settle(config);
    }

    pub fn within_budget(&self) -> bool {
        self.tally.as_ref().is_none_or(|tally| !tally.over())
    }

    pub fn refresh(&mut self, cats: &[CatEntry], config: &Config, ctx: GlobalContext<'_>) -> Task<Message> {
        self.stale = false;

        self.sync(cats, config, ctx)
    }

    pub fn picks(&self, config: &Config, map_id: i32) -> Option<(RoguelikePicks, Vec<SetupUnit>)> {
        let board = self.board.as_ref().filter(|board| board.map_id == map_id)?;
        let run = config.jailbreak.get(&map_id).and_then(Runs::current).map_or_else(Run::default, |run| board.settled(run));
        let roster = board.roster(&run).into_iter().map(|(starter, id)| if starter { RosterPick::Starter(id) } else { RosterPick::Recruit(id) }).collect();
        let fielded = board
            .deck(&run)
            .into_iter()
            .map(|(unit, standing)| SetupUnit {
                unit: unit as i32,
                form: standing.form as i32,
                level: u32::try_from(standing.level).unwrap_or(1),
                plus: u32::try_from(standing.plus).unwrap_or(0),
                talents: Vec::new(),
                orbs: Vec::new(),
            })
            .collect();

        let cannon = board.cannon(&run);

        let registered = board.registered(&run);

        Some((RoguelikePicks { roster, buffs: run.bonuses.into_iter().collect(), cannon, registered }, fielded))
    }

    fn settle(&mut self, config: &Config) {
        let Some(board) = self.board.as_ref() else {
            self.run = Run::default();
            self.lineup = Lineup::default();
            self.forms.clear();
            self.global.clear();
            self.cat.clear();
            self.saved.clear();
            self.tally = None;

            return;
        };
        let runs = config.jailbreak.get(&board.map_id).cloned().unwrap_or_default();

        self.run = runs.current().map_or_else(Run::default, |run| board.settled(run));
        self.lineup = board.lineup(&self.run);
        self.forms = board.cats.iter().map(|cat| (cat.unit, board.standing(&self.run, cat.unit).form)).collect();
        self.global = grouped(board, &self.run, &self.global_query, true);
        self.cat = grouped(board, &self.run, &self.cat_query, false);
        self.tally = board.tally(&self.run, self.stage);
        self.selected = runs.selected;
        self.named.clone_from(&self.run.name);

        let wanted = self.search.to_lowercase();

        self.saved = runs
            .saved
            .into_iter()
            .enumerate()
            .filter(|(_, run)| wanted.is_empty() || run.name.to_lowercase().contains(&wanted))
            .map(|(index, run)| Saved { index, count: format!("{}/{LINEUP_SLOTS}", board.fielded(&run).len()), name: run.name })
            .collect();

        if self.viewing.is_some_and(|unit| self.lineup.find(unit).is_none()) {
            self.viewing = None;
            self.combos.focus(None);
        }
    }

    fn sync(&mut self, cats: &[CatEntry], config: &Config, ctx: GlobalContext<'_>) -> Task<Message> {
        self.settle(config);

        let Some(board) = self.board.as_ref() else {
            return Task::none();
        };
        let vfs = &ctx.vault.vfs;
        let mut wanted: Vec<(u32, usize)> = self.forms.iter().map(|(unit, form)| (*unit, *form)).collect();

        wanted.sort_unstable();

        let adopted = if self.adopted.as_ref() == Some(&wanted) {
            Task::none()
        } else {
            let listed: Vec<CatEntry> = cats
                .iter()
                .filter(|cat| board.cat(cat.id).is_some())
                .map(|cat| {
                    let mut shown = cat.clone();
                    let form = self.forms.get(&cat.id).copied().unwrap_or(0);

                    for path in shown.banner_paths.iter_mut().skip(form.saturating_add(1)) {
                        *path = None;
                    }

                    shown
                })
                .collect();

            self.adopted = Some(wanted);
            self.inspector.set_banner_form(usize::MAX);
            self.inspector.adopt_cats(&listed, ctx.vault).map(Message::Cat)
        };

        if let Some(cell) = self.viewing.and_then(|unit| self.lineup.find(unit)) {
            self.view_unit(cell, ctx);
        }

        self.combos.ensure(ctx, cats, Some(&self.lineup));
        self.combos.refresh(ctx, Some(&self.lineup));

        if self.base_icons.is_empty() {
            let sheets = formats::sheets(vfs, rules::BASE_SHEET);
            let wanted = self.board.iter().flat_map(|board| board.bonuses.iter()).filter(|bonus| bonus.sheet == rules::BASE_SHEET).filter_map(|bonus| match bonus.icon {
                AbilityIcon::Standard(cut) => Some(cut),
                AbilityIcon::Custom(_) | AbilityIcon::None => None,
            });

            for cut in wanted {
                if let Some(cropped) = sheets.iter().find_map(|sheet| sheet.crop(cut)) {
                    self.base_icons.entry(cut).or_insert_with(|| Handle::from_rgba(cropped.width(), cropped.height(), cropped.into_raw()));
                }
            }
        }

        self.icons.clear();

        for member in self.lineup.slots.iter().chain(self.lineup.bench.iter().flatten()) {
            let path = match self.inspector.cat(member.id) {
                Some(cat) => scanner::deploy_icon(vfs, cat, member.form),
                None => scanner::orphan_icon(vfs, member.id, member.form),
            };

            if let Some(icon) = path.and_then(|path| header_icon::load(&self.decoded, &vfs.source(&path))) {
                self.icons.insert((member.id, member.form), icon.handle);
            }
        }

        adopted
    }

    fn edit(&mut self, config: &mut Config, change: impl FnOnce(&Board, &mut Run)) {
        let Some(board) = self.board.as_ref() else {
            return;
        };
        let run = config.jailbreak.entry(board.map_id).or_default().current_mut();

        board.settle(run);
        change(board, run);

        self.stale = true;
    }

    fn shelve(&mut self, config: &mut Config, change: impl FnOnce(&mut Runs)) {
        let Some(board) = self.board.as_ref() else {
            return;
        };

        change(config.jailbreak.entry(board.map_id).or_default());

        self.stale = true;
    }

    fn close(&mut self) {
        self.viewing = None;
        self.drag = Drag::Idle;
        self.plan = None;
        self.combos.focus(None);
    }

    fn landing(&self, at: Point) -> Landing {
        if self.area.width <= 0.0 {
            return Landing::Card;
        }

        let card_left = LEFT_WIDTH + ((self.area.width - LEFT_WIDTH - RIGHT_WIDTH - self.metrics.deck_width()) / 2.0).max(0.0);

        self.metrics.landing(Point::new(at.x - card_left, at.y - TOP_PAD))
    }

    fn planned(&self, lifted: Option<Cell>, landing: Landing) -> Option<Arrangement> {
        let Landing::Cell(target, edge) = landing else {
            return None;
        };

        self.board.as_ref()?.arrange(&self.lineup, lifted, target, edge)
    }

    fn view_unit(&mut self, cell: Cell, ctx: GlobalContext<'_>) {
        let Some(member) = self.lineup.get(cell) else {
            return;
        };

        let grants = self.board.as_ref().map(|board| board.grants(&self.run, member.id)).unwrap_or_default();

        let moved = self.shown.as_ref().is_none_or(|shown| shown.id != member.id || shown.form != member.form || shown.level != member.level);

        if moved || self.viewing != Some(member.id) {
            self.inspector.reveal(member.id, member.form, &ctx.vault.vfs);
            self.inspector.set_export_scope(format!("jailbreak-{}", self.board.as_ref().map_or(0, |board| board.map_id)));
            self.inspector.inspect(member.id, member.form, &member.level, &member.talents);
        }

        self.inspector.grant(member.id, grants);
        self.combos.focus(Some((member.id, member.form)));
        self.viewing = Some(member.id);
        self.shown = Some(member.clone());
    }

    pub fn update(&mut self, message: Message, cats: &[CatEntry], settings: &mut Settings, app_state: &mut AppState, ctx: GlobalContext<'_>) -> Task<Message> {
        let task = self.update_inner(message, settings, app_state, ctx);

        if !std::mem::take(&mut self.stale) {
            return task;
        }

        Task::batch([task, self.sync(cats, &app_state.sandbox.config, ctx)])
    }

    fn update_inner(&mut self, message: Message, settings: &mut Settings, app_state: &mut AppState, ctx: GlobalContext<'_>) -> Task<Message> {
        match message {
            Message::Cat(msg) => {
                match msg.picked() {
                    Some(Picked::Pressed(id)) => {
                        self.drag = Drag::Pressed { cargo: Cargo::Fresh(id), since: Instant::now(), from: None };

                        return Task::none();
                    }
                    Some(Picked::Chosen(id)) => {
                        self.edit(&mut app_state.sandbox.config, |board, run| board.add(run, id));

                        return Task::none();
                    }
                    None => (),
                }

                let browsing = matches!(msg, cat::Message::SearchChanged(_) | cat::Message::List(_) | cat::Message::Filter(_));

                if !browsing && !msg.views_only() {
                    return Task::none();
                }

                self.inspector.update(msg, settings, app_state, ctx).map(Message::Cat)
            }
            Message::Combos(msg) => {
                self.combos.update(msg);

                Task::none()
            }
            Message::OpenGlobal => {
                self.global_open = self.board.is_some();
                self.stale = true;

                Task::none()
            }
            Message::OpenCat => {
                self.cat_open = self.board.is_some();
                self.stale = true;

                Task::none()
            }
            Message::GlobalPopup(msg) => {
                if self.global_popup.update(msg, GLOBAL_POPUP) {
                    self.global_open = false;
                }

                Task::none()
            }
            Message::CatPopup(msg) => {
                if self.cat_popup.update(msg, CAT_POPUP) {
                    self.cat_open = false;
                }

                Task::none()
            }
            Message::UnitPopup(msg) => {
                if self.unit_popup.update(msg, UNIT_POPUP) {
                    self.viewing = None;
                    self.combos.focus(None);
                }

                Task::none()
            }
            Message::Measured(size) => {
                self.area = size;
                self.metrics = Metrics::fitting_actions(size);

                Task::none()
            }
            Message::PressCell(cell) => {
                if self.lineup.get(cell).is_none() {
                    return Task::none();
                }

                self.drag = Drag::Pressed { cargo: Cargo::Held(cell), since: Instant::now(), from: None };

                self.holding.set_after(cell, Message::Held, HOLD)
            }
            Message::DragMove(at) => {
                self.drag = match self.drag {
                    Drag::Pressed { cargo, since, from } => {
                        let origin = from.unwrap_or(at);
                        let strayed = (at.x - origin.x).abs() > SLACK || (at.y - origin.y).abs() > SLACK;
                        let ripe = matches!(cargo, Cargo::Fresh(_)) && since.elapsed() >= RIPE;

                        if strayed || ripe {
                            self.holding.clear();

                            let landing = self.landing(at);
                            let lifted = match cargo {
                                Cargo::Held(cell) => Some(cell),
                                Cargo::Fresh(id) => self.lineup.find(id),
                            };

                            self.plan = self.planned(lifted, landing);

                            Drag::Moving { cargo, lifted, at, landing }
                        } else {
                            Drag::Pressed { cargo, since, from: Some(origin) }
                        }
                    }
                    Drag::Moving { cargo, lifted, landing: before, .. } => {
                        let landing = self.landing(at);

                        if landing != before {
                            self.plan = self.planned(lifted, landing);
                        }

                        Drag::Moving { cargo, lifted, at, landing }
                    }
                    Drag::Idle => Drag::Idle,
                };

                Task::none()
            }
            Message::DragCancel => {
                self.drag = Drag::Idle;
                self.plan = None;
                self.holding.clear();

                Task::none()
            }
            Message::Held => {
                let held = self.holding.get().copied();

                self.holding.expire();

                if let (Some(cell), Drag::Pressed { cargo: Cargo::Held(pressed), .. }) = (held, self.drag)
                    && cell == pressed
                    && let Some(unit) = self.lineup.get(cell).map(|member| member.id)
                {
                    self.drag = Drag::Idle;
                    self.plan = None;
                    self.clicked = None;
                    self.edit(&mut app_state.sandbox.config, |board, run| board.toggle_registered(run, unit));
                }

                Task::none()
            }
            Message::DragEnd => {
                let settled = std::mem::take(&mut self.drag);
                let plan = self.plan.take();

                self.holding.clear();

                match settled {
                    Drag::Pressed { cargo: Cargo::Fresh(id), .. } => self.edit(&mut app_state.sandbox.config, |board, run| board.add(run, id)),
                    Drag::Pressed { cargo: Cargo::Held(cell), .. } => {
                        let now = Instant::now();
                        let doubled = self.clicked.is_some_and(|(held, at)| held == cell && now.duration_since(at) <= DOUBLE_CLICK);

                        self.clicked = (!doubled).then_some((cell, now));

                        if doubled {
                            self.view_unit(cell, ctx);
                        }
                    }
                    Drag::Moving { cargo, landing: Landing::Cell(..), lifted, .. } if let Some(plan) = plan => {
                        let fresh = match cargo {
                            Cargo::Fresh(id) if lifted.is_none() => Some(id),
                            _ => None,
                        };

                        self.edit(&mut app_state.sandbox.config, |board, run| board.commit(run, &plan, fresh));
                    }
                    Drag::Moving { cargo: Cargo::Held(from), landing: Landing::Off, .. } => {
                        self.edit(&mut app_state.sandbox.config, |board, run| board.discard(run, from));
                    }
                    _ => (),
                }

                Task::none()
            }
            Message::Raise(id, level) => {
                self.edit(&mut app_state.sandbox.config, |board, run| board.raise(run, id, level));

                Task::none()
            }
            Message::GlobalQuery(query) => {
                self.global_query = query;
                self.stale = true;

                Task::none()
            }
            Message::CatQuery(query) => {
                self.cat_query = query;
                self.stale = true;

                Task::none()
            }
            Message::Create => {
                self.shelve(&mut app_state.sandbox.config, Runs::create);
                self.close();

                Task::none()
            }
            Message::Delete => {
                if !self.deleting.is_set() {
                    return self.deleting.set((), Message::DeleteExpired);
                }

                self.deleting.clear();
                self.shelve(&mut app_state.sandbox.config, Runs::delete);
                self.close();

                Task::none()
            }
            Message::DeleteExpired => {
                self.deleting.expire();

                Task::none()
            }
            Message::Load(index) => {
                self.deleting.clear();
                self.shelve(&mut app_state.sandbox.config, |runs| runs.load(index));
                self.close();

                Task::none()
            }
            Message::Rename(name) => {
                self.shelve(&mut app_state.sandbox.config, |runs| runs.current_mut().name = name);

                Task::none()
            }
            Message::Search(query) => {
                self.search = query;
                self.stale = true;

                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let lifted = match self.drag {
            Drag::Moving { lifted, .. } => lifted,
            _ => None,
        };
        let plan = self.plan.as_ref();
        let over = self.tally.as_ref().is_some_and(|tally| tally.bonuses_over);
        let actions = row![
            self.action(rules::GLOBAL_LABEL, Message::OpenGlobal, over),
            self.action(rules::CAT_LABEL, Message::OpenCat, over),
        ]
            .spacing(self.metrics.gap);
        let deck = deck_card(self.metrics, rules::BENCH_NOTE, Some(actions.into()), |cell| {
            let source = plan.map_or_else(|| (lifted != Some(cell)).then_some(Placed::Kept(cell)), |plan| plan.get(cell));

            match source {
                Some(Placed::Kept(source)) => self.cell(cell, self.lineup.get(source), false),
                Some(Placed::Carried) => self.cell(cell, None, true),
                None => self.cell(cell, None, false),
            }
        });
        let center = container(
            column![deck, self.combos.view((self.area.width - LEFT_WIDTH - RIGHT_WIDTH - CARD_GAP * 2.0).max(0.0)).map(Message::Combos)]
                .spacing(CARD_GAP)
                .align_x(Horizontal::Center),
        )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding { top: TOP_PAD, right: CARD_GAP, bottom: CARD_GAP, left: CARD_GAP })
            .align_x(Horizontal::Center);
        let footer = self.tally.as_ref().map(|tally| {
            let style = if tally.recruits_over { theme::danger_block_container } else { theme::success_block_container };
            let block: Element<'_, cat::Message> = container(text(tally.recruits.as_str()).size(TALLY_SIZE)).width(Length::Fill).padding(TALLY_PADDING).align_x(Horizontal::Center).style(style).into();
            let block = match tally.recruits_note() {
                Some(note) => hover_hint(block, note),
                None => block,
            };

            column![Space::new().height(Length::Fixed(FOOTER_GAP)), block].into()
        });
        let content = row![self.inspector.sidebar_view_with(footer).map(Message::Cat), center, self.view_saved()].width(Length::Fill).height(Length::Fill);
        let mut layers = stack![sensor(content).on_show(Message::Measured).on_resize(Message::Measured)];

        if self.drag != Drag::Idle {
            let ghost: Element<'_, Message> = match self.drag {
                Drag::Moving { cargo, at, .. } => self.ghost(cargo, at),
                _ => Space::new().width(Length::Fill).height(Length::Fill).into(),
            };

            layers = layers.push(mouse_area(ghost).interaction(mouse::Interaction::Grabbing).on_move(Message::DragMove).on_release(Message::DragEnd));
        }

        editor::suppress(layers, true)
    }

    pub fn global_view(&self, window: Size) -> Option<Element<'_, Message>> {
        let board = self.board.as_ref().filter(|_| self.global_open)?;

        Some(self.global_popup.view(rules::GLOBAL_TITLE, GLOBAL_POPUP, window, Message::GlobalPopup, move || self.bonus_content(board, &self.global, &self.global_query, Message::GlobalQuery), None))
    }

    pub fn cat_view(&self, window: Size) -> Option<Element<'_, Message>> {
        let board = self.board.as_ref().filter(|_| self.cat_open)?;

        Some(self.cat_popup.view(rules::CAT_TITLE, CAT_POPUP, window, Message::CatPopup, move || self.bonus_content(board, &self.cat, &self.cat_query, Message::CatQuery), None))
    }

    pub fn unit_view<'a>(&'a self, window: Size, settings: &'a Settings, app_state: &'a AppState, ctx: GlobalContext<'a>) -> Option<Element<'a, Message>> {
        let member = self.lineup.get(self.lineup.find(self.viewing.filter(|_| self.board.is_some())?)?)?;
        let title = self.inspector.cat(member.id).map_or("Unit", |cat| cat.names.get(member.form).and_then(Option::as_deref).unwrap_or("Unit"));

        Some(self.unit_popup.view(title, UNIT_POPUP, window, Message::UnitPopup, move || self.inspector.detail_view(settings, app_state, ctx, None).map(Message::Cat), None))
    }

    pub fn export_view(&self, window: Size) -> Option<Element<'_, Message>> {
        (self.unit_open() && self.inspector.export_popup_visible()).then(|| self.inspector.export_popup_view(window)).flatten().map(|view| view.map(Message::Cat))
    }

    pub fn filter_view<'a>(&'a self, window: Size, names: &'a NameBook) -> Option<Element<'a, Message>> {
        self.board.as_ref().and_then(|_| self.inspector.filter_popup_view(window, names)).map(|view| view.map(Message::Cat))
    }

    fn action<'a>(&self, label: &'a str, on_press: Message, over: bool) -> Element<'a, Message> {
        let scale = self.metrics.scale;

        button(theme::centered_text(label).size(self.metrics.text(ACTION_SIZE)).width(Length::Fill))
            .width(Length::Fixed(ACTION_WIDTH * scale))
            .height(Length::Fixed(ACTION_HEIGHT * scale))
            .style(if over { theme::danger_button } else { theme::primary_button })
            .on_press(on_press)
            .into()
    }

    fn tally_block<'a>(&self, tally: &'a Tally) -> Element<'a, Message> {
        let style = if tally.bonuses_over { theme::danger_block_container } else { theme::success_block_container };
        let block: Element<'a, Message> = container(text(tally.bonuses.as_str()).size(TEXT_SIZE)).padding(TALLY_PADDING).style(style).into();

        match tally.bonuses_note() {
            Some(note) => hover_hint(block, note),
            None => block,
        }
    }

    fn view_saved(&self) -> Element<'_, Message> {
        let delete = if self.deleting.is_set() { CONFIRM_SHORT_LABEL } else { "Delete" };
        let actions = column![
            button(theme::centered_text("New").size(NAME_SIZE).width(Length::Fill)).width(Length::Fill).style(theme::success_button).on_press(Message::Create),
            button(theme::centered_text(delete).size(NAME_SIZE).width(Length::Fill)).width(Length::Fill).style(theme::danger_button).on_press(Message::Delete),
        ]
            .spacing(6);
        let search = text_input("Search Lineup...", &self.search).on_input(Message::Search).padding(4).size(NAME_SIZE).style(theme::rounded_input);
        let rename = text_input("Lineup Name", &self.named).on_input(Message::Rename).padding(4).size(NAME_SIZE).style(theme::rounded_input);
        let mut listed = Column::new().spacing(4);

        for saved in &self.saved {
            let label = container(
                row![
                    text(saved.name.as_str()).size(NAME_SIZE).width(Length::Fill).wrapping(text::Wrapping::None),
                    weak(saved.count.as_str()).size(LEVEL_SIZE),
                ]
                    .spacing(6)
                    .align_y(Vertical::Center),
            )
                .padding([6, 8])
                .width(Length::Fill)
                .clip(true);

            listed = listed.push(list_row(label, saved.index == self.selected, true, Length::Fill, Message::Load(saved.index)));
        }

        container(column![actions, search, smooth_scroll(scrollable(listed).width(Length::Fill).height(Length::Fill)), rename].spacing(8))
            .width(Length::Fixed(RIGHT_WIDTH))
            .height(Length::Fill)
            .padding(8)
            .style(theme::list_panel_container_right)
            .into()
    }

    fn ghost(&self, cargo: Cargo, at: Point) -> Element<'_, Message> {
        let metrics = self.metrics;
        let handle = match cargo {
            Cargo::Held(cell) => self.lineup.get(cell).and_then(|member| self.icons.get(&(member.id, member.form))).cloned(),
            Cargo::Fresh(id) => self.inspector.cat(id).and_then(|cat| {
                let form = self.forms.get(&id).copied().unwrap_or(0);

                cat.deploy_icon_paths
                    .get(form)
                    .and_then(Option::as_ref)
                    .and_then(|path| header_icon::load(&self.decoded, &Source::disk(path)))
                    .map(|icon| icon.handle)
            }),
        };
        let (width, height) = (Length::Fixed(metrics.cell_width), Length::Fixed(metrics.cell_height));
        let carried: Element<'_, Message> = handle.map_or_else(
            || Space::new().width(width).height(height).into(),
            |handle| iced_image(handle).width(width).height(height).opacity(GHOST_ALPHA).into(),
        );
        let placed = Padding::default().top((at.y - metrics.cell_height / 2.0).max(0.0)).left((at.x - metrics.cell_width / 2.0).max(0.0));

        container(carried).padding(placed).width(Length::Fill).height(Length::Fill).into()
    }

    fn cell<'a>(&'a self, cell: Cell, member: Option<&'a Member>, aimed: bool) -> Element<'a, Message> {
        let metrics = self.metrics;
        let (width, height) = (Length::Fixed(metrics.cell_width), Length::Fixed(metrics.cell_height));

        let frame = cell_frame(aimed);

        let Some(member) = member else {
            return container(Space::new()).width(width).height(height).style(frame).into();
        };

        let recruits_over = self.tally.as_ref().is_some_and(|tally| tally.recruits_over);
        let flagged = recruits_over && matches!(cell, Cell::Slot(_)) && self.board.as_ref().is_some_and(|board| board.recruited(member.id));

        let face: Element<'a, Message> = self.icons.get(&(member.id, member.form)).map_or_else(
            || container(text(format!("{:03}-{}", member.id, member.form + 1)).size(LEVEL_SIZE)).center_x(Length::Fill).center_y(Length::Fill).into(),
            |handle| iced_image(handle.clone()).width(width).height(height).into(),
        );
        let (level, plus) = member.levels(None);
        let shown = if plus > 0 { format!("Lv{level}+{plus}") } else { format!("Lv{level}") };
        let badge = container(cell_badge(shown, metrics.text(BADGE_SIZE))).width(Length::Fill).height(Length::Fill).align_x(Horizontal::Right).align_y(Vertical::Top);
        let mut tile = stack![container(face).width(width).height(height).style(frame)].width(width).height(height);

        if flagged {
            tile = tile.push(container(Space::new()).width(width).height(height).style(cell_outline));
        }

        tile = tile.push(badge);

        if self.run.registered.contains(&member.id) {
            let registered = cell_badge(rules::REGISTERED.to_owned(), metrics.text(BADGE_SIZE));

            tile = tile.push(container(registered).width(Length::Fill).height(Length::Fill).align_x(Horizontal::Left).align_y(Vertical::Top));
        }


        mouse_area(tile).interaction(mouse::Interaction::Grab).on_press(Message::PressCell(cell)).into()
    }

    fn bonus_content<'a>(&'a self, board: &'a Board, groups: &'a [Group], query: &'a str, on_query: fn(String) -> Message) -> Element<'a, Message> {
        let mut listed = Column::new().spacing(GROUP_SPACING).width(Length::Fill);

        for group in groups {
            let mut tiles = Row::new().spacing(LIST_SPACING);

            for offer in &group.offers {
                if let Some(bonus) = board.bonuses.get(offer.index) {
                    tiles = tiles.push(self.tile(bonus, offer));
                }
            }

            listed = listed.push(column![theme::bold_text(group.title.as_str()).size(TITLE_SIZE), tiles.wrap().vertical_spacing(LIST_SPACING)].spacing(LIST_SPACING));
        }

        if groups.is_empty() {
            listed = listed.push(weak(NO_BONUSES));
        }

        if self.tally.is_some() {
            listed = listed.push(Space::new().height(Length::Fixed(TALLY_CLEARANCE)));
        }

        let rail = scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(RAIL_WIDTH).margin(RAIL_MARGIN));
        let scroll = smooth_scroll(scrollable(listed).direction(rail).height(Length::Fill));
        let mut layered = stack![scroll].width(Length::Fill).height(Length::Fill);

        if let Some(tally) = self.tally.as_ref() {
            layered = layered.push(container(self.tally_block(tally)).width(Length::Fill).height(Length::Fill).align_x(Horizontal::Center).align_y(Vertical::Bottom).padding(TALLY_MARGIN));
        }

        column![
            text_input("Search bonuses", query).on_input(on_query).size(TEXT_SIZE).style(theme::rounded_input),
            layered,
        ]
            .spacing(LIST_SPACING)
            .padding(PANE_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn tile<'a>(&'a self, bonus: &'a Bonus, offer: &'a Offer) -> Element<'a, Message> {
        let id = bonus.id;
        let drawn = match bonus.icon {
            AbilityIcon::Standard(cut) if bonus.sheet == rules::ABILITY_SHEET => self.bonus_icons.handle(cut, self.inspector.img015()),
            AbilityIcon::Standard(cut) => self.base_icons.get(&cut).cloned(),
            AbilityIcon::Custom(_) | AbilityIcon::None => None,
        };
        let icon: Element<'a, Message> = drawn.map_or_else(
            || fallback_icon(bonus.fallback),
            |handle| iced_image(handle).width(Length::Fixed(ICON_SIZE)).height(Length::Fixed(ICON_SIZE)).into(),
        );
        let tag: Element<'a, Message> = if bonus.permanent { weak(rules::PERMANENT).into() } else { Space::new().into() };
        let level = column![
            row![
                slider(0..=bonus.limit, offer.level, move |level| Message::Raise(id, level)).width(Length::Fixed(SLIDER_WIDTH)),
                text(offer.count.as_str()).size(TEXT_SIZE).width(Length::Fixed(COUNT_WIDTH)).align_x(Horizontal::Right),
            ]
                .spacing(LIST_SPACING)
                .align_y(Vertical::Center),
            tag,
        ];

        container(row![hover_hint(icon, offer.label.as_str()), level].spacing(LIST_SPACING).align_y(Vertical::Center))
            .padding(TILE_PADDING)
            .style(theme::card_container_muted)
            .into()
    }
}

fn grouped(board: &Board, run: &Run, query: &str, global: bool) -> Vec<Group> {
    let query = query.trim().to_lowercase();
    let mut groups: Vec<Group> = Vec::new();

    for (index, bonus) in board.bonuses.iter().enumerate() {
        if bonus.target.is_none() != global || !board.offers(run, bonus) {
            continue;
        }

        if !matches(&board.label(run, bonus), &query) {
            continue;
        }

        let title = board.title(run, bonus);
        let level = run.level(bonus.id);
        let offer = Offer { index, level, count: format!("{level}/{}", bonus.limit), label: board.detail(run, bonus) };

        match groups.iter_mut().find(|group| group.title == title) {
            Some(group) => group.offers.push(offer),
            None => groups.push(Group { title, offers: vec![offer] }),
        }
    }

    groups
}

fn weak<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).size(NOTE_SIZE).style(|theme: &Theme| text::Style { color: Some(theme::weak_text_color(theme)) })
}
