mod battle;
mod config;
mod jailbreak;
mod lineup;
mod orbs;
mod replay;
mod state;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::info;

use codex::chapter::Category;
use emulator::runtime::{BattleOptions, CASTLE_PART, CatGod as EngineGod, Setup, SetupUnit, StageEntry, StageVariant, TechLevel, TutorialFlags, offers_cat_god, usable_items};
use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::sandbox::altar::Altars;
use foundry::domains::sandbox::base::Parts;
use foundry::domains::sandbox::config::{self as sandbox_config, CatGod, Gimmicks};
use foundry::domains::sandbox::jailbreak::{Board, Grants};
use foundry::domains::sandbox::replay::VANILLA_APP;
use foundry::domains::sandbox::rules::Rules;
use foundry::domains::sandbox::{Arrangement, Cell, Member, TECHS, scored};
use foundry::domains::stage::lottery::Lottery;
use foundry::domains::stage::scanner::StageBundle;
use foundry::domains::stage::{GlobalMapId, GlobalStageId, StageRegistry, navigate};

use crate::catalog::Load;
use crate::host::{Arena, Command, Label, Launch, Standing};
use crate::images::Images;
use crate::pages::Context;
use crate::platform;
use crate::roster::Roster;
use crate::{AppWindow, DeckCell, Page, Sandbox, Shell, bridge};

use self::orbs::Book;
use self::state::{Device, Saved, Store, Tab, Volume};

const ACKNOWLEDGMENT: &str = "The purpose of this agreement is to ensure that you, the User, are aware of the potential quirks regarding Sandbox.\nBy accepting this agreement, you must have read, understood, and accepted the following facts about Sandbox:\n\n\u{2022} Sandbox may have known but subtle inaccuracies, mechanical bugs, or logical quirks when compared to the real game.\n\u{2022} Sandbox may not be updated to support features and/or mechanics from the latest game version(s) within a timely manner.\n\u{2022} Sandbox is representative of the vanilla game logic and it's capabilities only, with no custom functionality added or planned.\n\u{2022} Sandbox is not an avenue that replaces the appeal and purpose of the game in any capacity, whether that be vanilla or modded.\n\nYou can accept the agreement by tapping the \"Agree\" button below. Selecting \"Disagree\" will return you to the Home page.";
const AGREEMENT: &str = "v1";
const OUTBREAK_TAG: &str = "Outbreak";
const INVASION_TAG: &str = "Invasion";
const STATUS_MS: u64 = 2000;

pub struct Staging {
    pub entry: StageEntry,
    pub usable: [bool; 6],
    pub rules: Rules,
    pub altar: Option<u32>,
    pub gimmicks: Gimmicks,
    pub jail: Option<(i32, i32)>,
    pub map: String,
    pub stage: String,
}

pub struct State {
    pub store: Store,
    pub cats: Option<Rc<Vec<CatEntry>>>,
    pub bundle: Option<Rc<StageBundle>>,
    pub book: Option<Rc<Book>>,
    pub parts: Option<Rc<Parts>>,
    pub roster: Rc<Roster>,
    pub editing: Option<(Cell, u32)>,
    pub resting: (Vec<DeckCell>, Vec<DeckCell>),
    pub plan: Option<Arrangement>,
    pub combos: lineup::ComboFilters,
    pub orb_slot: Option<usize>,
    pub deleting: bool,
    pub saved_search: String,
    pub folds: HashSet<usize>,
    pub replay: replay::Part,
    pub battle: battle::Part,
    pub arena: Option<Arena>,
    pub arena_ticket: Option<bridge::Ticket>,
    pub status: String,
    pub timer: Timer,
    pub delete_timer: Timer,
    pub entered: bool,
    pub jail: Option<(jailbreak::Key, Option<Rc<Board>>)>,
    pub jail_lineup: bool,
    pub jail_viewing: Option<u32>,
    pub jail_shown: Option<(Member, Rc<Grants>)>,
    pub jail_global: bool,
    pub jail_listed: Vec<(usize, usize)>,
}

pub type Shared = Rc<RefCell<State>>;

impl State {
    fn new(images: &Images) -> Self {
        let roster = Roster::new(images, Rc::new(|images: &Images, vault, path| (format!("cat-banner:{}", path.display()), images.cat_banner(vault, path))));

        Self {
            store: Store::load(),
            cats: None,
            bundle: None,
            book: None,
            parts: None,
            roster,
            editing: None,
            resting: (Vec::new(), Vec::new()),
            plan: None,
            combos: lineup::ComboFilters::default(),
            orb_slot: None,
            deleting: false,
            saved_search: String::new(),
            folds: HashSet::new(),
            replay: replay::Part::default(),
            battle: battle::Part::default(),
            arena: None,
            arena_ticket: None,
            status: String::new(),
            timer: Timer::default(),
            delete_timer: Timer::default(),
            entered: false,
            jail: None,
            jail_lineup: false,
            jail_viewing: None,
            jail_shown: None,
            jail_global: false,
            jail_listed: Vec::new(),
        }
    }

    pub fn saved(&self) -> std::cell::Ref<'_, Saved> {
        self.store.get()
    }

    pub fn cat(&self, id: u32) -> Option<CatEntry> {
        self.cats.as_ref()?.iter().find(|cat| cat.id == id).cloned()
    }
}

fn invaded_slot(registry: &StageRegistry, picked: &GlobalStageId, saved: &Saved) -> Option<u32> {
    let invasions = &saved.config.invasions;

    navigate::is_invasion(registry, picked).then_some(if picked.category == Category::ZombieOutbreaks { invasions.filibuster_outbreak } else { invasions.filibuster })
}

fn label(registry: &StageRegistry, picked: &GlobalStageId, saved: &Saved) -> (String, String) {
    let map_key = GlobalMapId { category: picked.category.clone(), map: picked.map };
    let map = registry.maps.get(&map_key).map_or_else(String::new, |map| match map.category {
        Category::ZombieOutbreaks => format!("{} ({OUTBREAK_TAG})", map.name),
        _ => map.name.clone(),
    });
    let name = registry.stages.get(picked).map(|stage| stage.name.as_str()).unwrap_or_default();
    let invaded = invaded_slot(registry, picked, saved).and_then(|slot| registry.stages.get(&GlobalStageId { category: picked.category.clone(), map: picked.map, stage: slot }));
    let stage = match invaded {
        Some(invaded) => format!("{} ({INVASION_TAG})", invaded.name),
        None if navigate::is_invader(registry, &map_key) => format!("{name} ({INVASION_TAG})"),
        None => name.to_owned(),
    };

    (map, stage)
}

fn staged_entry(context: &Rc<Context>, registry: &StageRegistry, picked: &GlobalStageId, crown: u8, saved: &Saved) -> Option<StageEntry> {
    let map = GlobalMapId { category: picked.category.clone(), map: picked.map };
    let global = registry.addresses.get(&map)?.global?;

    if let Some(target) = navigate::invaded_stage(registry, picked) {
        return Some(StageEntry {
            map_id: i32::try_from(target.map_id).ok()?,
            stage: i32::try_from(target.stage).ok()?,
            layout: None,
            crown: i32::from(target.crown),
            dungeon_stage: None,
            variant: StageVariant::ExInvasion,
        });
    }

    let map_id = i32::try_from(global).ok()?;
    let outbreak = picked.category == Category::ZombieOutbreaks;
    let invaded = invaded_slot(registry, picked, saved);
    let variant = match (outbreak, invaded.is_some()) {
        (false, false) => StageVariant::Plain,
        (true, false) => StageVariant::Outbreak,
        (false, true) => StageVariant::Invasion,
        (true, true) => StageVariant::ZombieInvasion,
    };
    let stage = invaded.unwrap_or(picked.stage);
    let layout = navigate::map_screen_slot(map_id, i32::try_from(stage).ok()?);
    let vault = context.engine.vault();
    let slot = Lottery::load(&vault.vfs).slot_for(map_id, layout);

    Some(StageEntry { map_id, stage: slot.unwrap_or(layout), layout: slot.map(|_| layout), crown: i32::from(crown), dungeon_stage: None, variant })
}

pub fn staging(state: &State, context: &Rc<Context>) -> Option<Staging> {
    let bundle = state.bundle.as_ref()?;
    let registry = &bundle.registry;
    let (picked, crown) = context.pages()?.stages.borrow().picked()?;
    let saved = state.saved();
    let entry = staged_entry(context, registry, &picked, crown, &saved)?;
    let stage = registry.stages.get(&picked);
    let map_key = GlobalMapId { category: picked.category.clone(), map: picked.map };
    let rules = stage.map(|stage| Rules::of(stage, registry.maps.get(&map_key), crown as i8)).unwrap_or_default();
    let vault = context.engine.vault();
    let global = registry.addresses.get(&map_key).and_then(|address| address.global).and_then(|global| i32::try_from(global).ok());
    let usable = global.map_or([true; 6], |map_id| usable_items(map_id, scored::score_stage_maps(&vault.vfs).contains(&map_id)));
    let castle = stage.and_then(|stage| i32::try_from(stage.anim_base_id).ok());
    let altars = Altars::load(&vault.vfs);
    let altar = castle.and_then(|castle| altars.level_cap(castle, saved.config.altar()));
    let special_rules = registry.maps.get(&map_key).and_then(|map| map.special_rules.as_ref());
    let gimmicks = Gimmicks {
        cat_god: offers_cat_god(entry),
        altar: castle.is_some_and(|castle| altars.seals(castle)),
        war_funds: special_rules.is_some_and(|rules| rules.draws_war_funds()),
        expenses: special_rules.is_some_and(|rules| rules.draws_expenses()),
        jailbreak: global.is_some_and(|map_id| foundry::domains::sandbox::jailbreak::is_challenge_map(&vault.vfs, map_id)),
    };
    let jail = global.filter(|_| gimmicks.jailbreak).map(|map_id| (map_id, i32::from(crown)));
    let (map, stage) = label(registry, &picked, &saved);

    Some(Staging { entry, usable, rules, altar, gimmicks, jail, map, stage })
}

fn build_setup(state: &State, context: &Rc<Context>, entry: StageEntry) -> Setup {
    let vault = context.engine.vault();
    let vfs = &vault.vfs;
    let parts = Parts::load(vfs);
    let orbs = vault.vds.cats.orbs(vfs);
    let orb_slots = vault.vds.cats.orb_slots(vfs);
    let saved = state.saved();
    let tutorial = &saved.config.tutorial;
    let mut setup = Setup {
        stage: entry,
        tutorial: TutorialFlags {
            battle_cleared: tutorial.battle_cleared,
            deck_seen: tutorial.deck_seen,
            two_rows_seen: tutorial.two_rows_seen,
            cat_god_seen: tutorial.cat_god_seen,
            shop_seen: tutorial.shop_seen,
        },
        ..Setup::default()
    };

    setup.lineup = saved
        .roster
        .current()
        .into_iter()
        .flat_map(|lineup| lineup.slots.iter())
        .map(|member| {
            let cat = state.cat(member.id);
            let (level, plus) = member.levels(cat.as_ref().and_then(|cat| cat.unitbuy.level_caps()));
            let groups = cat.and_then(|cat| cat.talent_data);
            let slots = orb_slots.get(&member.id).copied().unwrap_or(0);

            SetupUnit {
                unit: member.id as i32,
                form: member.form as i32,
                level,
                plus,
                talents: member
                    .talents
                    .iter()
                    .filter_map(|(index, level)| {
                        let group = groups.as_ref()?.groups.get(*index as usize)?;

                        Some((i32::from(group.ability_id), i32::from(*level)))
                    })
                    .collect(),
                orbs: member.worn(slots, &orbs).map(|(slot, orb)| (slot as i32, orb as i32)).collect(),
            }
        })
        .collect();

    setup.tech[0] = TechLevel { level: 1, plus: 0 };

    for (index, tech) in TECHS.iter().enumerate() {
        let (level, plus) = saved.config.tech(index);

        if let Some(held) = setup.tech.get_mut(tech.slot) {
            *held = TechLevel { level, plus };
        }
    }

    setup.treasures = saved.config.treasure.levels();
    setup.altar = saved.config.altar();
    setup.war_funds = saved.config.war_funds();
    setup.expenses = saved.config.expenses();
    setup.cat_god = match saved.config.cat_god {
        CatGod::Absent => EngineGod::Absent,
        CatGod::Present => EngineGod::Present,
        CatGod::Discounted => EngineGod::Discounted,
    };

    for (item, off) in saved.config.items_off.iter().enumerate() {
        if let Some(stocked) = setup.items.get_mut(item) {
            *stocked = !off;
        }
    }

    setup.speed_engaged = saved.config.start_speed.engaged();
    setup.cannon = saved.config.cannon.unwrap_or(0);
    setup.style = saved.config.style.unwrap_or(0);
    setup.foundation = saved.config.foundation.unwrap_or(0);

    let highest = |parts: &std::collections::BTreeMap<i32, i32>, id: i32| parts.get(&id).copied().unwrap_or(1);

    setup.parts.entry(setup.cannon).or_default().cannon = sandbox_config::level(&saved.config.cannon_level, highest(&parts.cannons, setup.cannon));
    setup.parts.entry(setup.style).or_default().style = sandbox_config::level(&saved.config.style_level, highest(&parts.styles, setup.style));
    setup.parts.entry(setup.foundation).or_default().foundation = sandbox_config::level(&saved.config.foundation_level, highest(&parts.foundations, setup.foundation));
    setup.parts.entry(CASTLE_PART).or_default().cannon = saved.config.castle(parts.castle);

    for levels in setup.parts.values_mut() {
        levels.cannon = levels.cannon.max(1);
    }

    if let Some(board) = state.jail.as_ref().and_then(|(_, board)| board.as_ref()).filter(|board| board.map_id == entry.map_id) {
        let (picks, fielded) = jailbreak::picks(&saved, board);

        setup.roguelike = picks;
        setup.lineup = fielded;
    }

    setup
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let state: Shared = Rc::new(RefCell::new(State::new(&context.images)));
    let sandbox = window.global::<Sandbox>();

    {
        let held = state.borrow();
        let unset = held.saved().device_type.is_none();

        if unset {
            let device = if platform::is_tablet() == Some(true) { Device::Tablet } else { Device::Phone };

            info!(?device, "The sandbox device type was detected");
            held.store.update(|saved| saved.device_type = Some(device));
        }
    }

    sandbox.set_terms(ACKNOWLEDGMENT.into());
    sandbox.set_fullscreen(state.borrow().saved().fullscreen);
    sandbox.set_roster(slint::ModelRc::from(Rc::clone(&state.borrow().roster) as Rc<dyn slint::Model<Data = crate::RosterRow>>));

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.catalogs.listen(Rc::new(move |ui: &AppWindow| sync(ui, &held, &ctx)));
    }

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));

        context.images.watch(Rc::new(move |ui: &AppWindow, key: &str| {
            if (key.starts_with("trim:") || key.starts_with("cut:")) && ui.global::<Shell>().get_page() == Page::Sandbox {
                refresh(ui, &held, &ctx);
            }
        }));
    }

    {
        let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
        let ticket = bridge::subscribe::<crate::host::Event>(move |ui, event| battle::event(ui, &held, &ctx, event));

        state.borrow_mut().arena_ticket = Some(ticket);
    }

    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(&state, context, window);
    sandbox.on_tab_set(move |index| {
        let tab = Tab::at(index);

        held.borrow().store.update(|saved| saved.tab = tab);

        if let Some(ui) = weak.upgrade() {
            if tab == Tab::Replay {
                replay::relist(&ui, &held, &ctx);
            }

            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    sandbox.on_agree(move || {
        held.borrow().store.update(|saved| saved.agreement = AGREEMENT.to_owned());

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (_, _, weak) = on(&state, context, window);
    sandbox.on_disagree(move || {
        if let Some(ui) = weak.upgrade() {
            ui.global::<Shell>().invoke_navigate(Page::Home);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    sandbox.on_play(move || {
        if let Some(ui) = weak.upgrade() {
            play(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(&state, context, window);
    sandbox.on_watch(move || {
        if let Some(ui) = weak.upgrade() {
            replay::watch(&ui, &held, &ctx, false);
        }
    });

    lineup::wire(window, &state, context);
    jailbreak::wire(window, &state, context);
    config::wire(window, &state, context);
    replay::wire(window, &state, context);
    battle::wire(window, &state, context);
    refresh(window, &state, context);

    state
}

fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if !state.borrow().entered {
        return;
    }

    let cats = context.catalogs.cats();
    let stages = context.catalogs.stages();

    if let Load::Ready(found) = &cats {
        let fresh = state.borrow().cats.as_ref().is_none_or(|held| !Rc::ptr_eq(held, found));

        if fresh {
            state.borrow_mut().cats = Some(Rc::clone(found));

            let forms = found.iter().map(|cat| (cat.id, cat.forms)).collect();
            let moved = {
                let held = state.borrow();
                let mut moved = 0;

                held.store.update(|saved| moved = saved.roster.settle_forms(&forms));
                moved
            };

            if moved > 0 {
                info!(moved, "Lineup members fell back to a form the data still has");
            }

            lineup::fill_roster(ui, state, context);
        }
    }

    if let Load::Ready(bundle) = &stages {
        let fresh = state.borrow().bundle.as_ref().is_none_or(|held| !Rc::ptr_eq(held, bundle));

        if fresh {
            state.borrow_mut().bundle = Some(Rc::clone(bundle));
        }
    }

    refresh(ui, state, context);
}

pub fn entered(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    state.borrow_mut().entered = true;

    if state.borrow().arena.is_none() {
        let ticket = state.borrow().arena_ticket;

        if let Some(ticket) = ticket {
            state.borrow_mut().arena = Arena::spawn(ui.as_weak(), ticket, context.engine.vault());
        }
    }

    let config = context.state.settings().scanner_config(context.engine.mounted());

    context.catalogs.load_stages(ui, &context.engine, config);
    sync(ui, state, context);

    if state.borrow().saved().tab == Tab::Replay {
        replay::relist(ui, state, context);
    }
}

pub fn left(state: &Shared) {
    let held = state.borrow();

    if let Some(arena) = &held.arena
        && held.battle.standing.phase != crate::host::Phase::Idle
    {
        arena.send(Command::Stop);
    }
}

pub fn invalidate(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    {
        let mut held = state.borrow_mut();

        held.book = None;
        held.parts = None;
        held.cats = None;
        held.bundle = None;

        if let Some(arena) = &held.arena {
            arena.send(Command::Vault(context.engine.vault()));
        }
    }

    if ui.global::<Shell>().get_page() == Page::Sandbox {
        sync(ui, state, context);
    }
}

pub fn book(state: &Shared, context: &Rc<Context>) -> Option<Rc<Book>> {
    if let Some(book) = &state.borrow().book {
        return Some(Rc::clone(book));
    }

    let globals = context.globals()?;
    let vault = context.engine.vault();
    let book = Rc::new(Book::load(&vault, &globals));

    state.borrow_mut().book = Some(Rc::clone(&book));

    Some(book)
}

pub fn parts(state: &Shared, context: &Rc<Context>) -> Rc<Parts> {
    if let Some(parts) = &state.borrow().parts {
        return Rc::clone(parts);
    }

    let vault = context.engine.vault();
    let parts = Rc::new(Parts::load(&vault.vfs));

    state.borrow_mut().parts = Some(Rc::clone(&parts));
    parts
}

pub fn report(ui: &AppWindow, state: &Shared, context: &Rc<Context>, text: String) {
    state.borrow_mut().status = text;

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let weak = ui.as_weak();

    state.borrow().timer.start(TimerMode::SingleShot, Duration::from_millis(STATUS_MS), move || {
        landing.borrow_mut().status.clear();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &landing, &ctx);
        }
    });
    refresh(ui, state, context);
}

fn options(saved: &Saved) -> BattleOptions {
    BattleOptions { music: Volume::nearest(saved.music_volume).percent(), effects: Volume::nearest(saved.effects_volume).percent(), two_rows: saved.two_rows, vibrate: saved.vibrate }
}

fn play(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let Some(staging) = staging(&state.borrow(), context) else {
        return;
    };
    let fielded = state.borrow().saved().roster.current().is_some_and(|lineup| !lineup.slots.is_empty());

    if !fielded || state.borrow().cats.is_none() {
        return;
    }

    let setup = build_setup(&state.borrow(), context, staging.entry);
    let (width, height) = battle::bounds(ui);
    let settings = context.state.settings();
    let launch = {
        let held = state.borrow();
        let saved = held.saved();

        Launch {
            setup,
            options: options(&saved),
            phone: saved.device() == Device::Phone,
            lag: saved.lag,
            width,
            height,
            label: Label { app: context.engine.mounted().unwrap_or_else(|| VANILLA_APP.to_owned()), map: staging.map, stage: staging.stage },
            record: !settings.sandbox.disable_replays,
            auto_save: settings.sandbox.auto_save_bcv.then_some(settings.sandbox.saved_bcv_name),
        }
    };

    battle::launch(ui, state, context, launch);
}

pub fn refresh(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = ui.global::<Sandbox>();
    let load = context.catalogs.cats();
    let ready = state.borrow().cats.is_some() && state.borrow().bundle.is_some();

    sandbox.set_ready(ready);

    if !ready {
        sandbox.set_status_title(if load.scanning() { "Scanning Cats...".into() } else { "Loading Cats...".into() });
        sandbox.set_status_detail(load.detail().unwrap_or_default().into());
    }

    let (tab, agreed) = {
        let held = state.borrow();
        let saved = held.saved();

        (saved.tab, saved.agreement == AGREEMENT)
    };
    let staged = staging(&state.borrow(), context);
    let fielded = state.borrow().saved().roster.current().is_some_and(|lineup| !lineup.slots.is_empty());

    sandbox.set_tab(tab.index());
    sandbox.set_agreed(agreed);
    sandbox.set_status(state.borrow().status.as_str().into());
    sandbox.set_lineup_note(staged.as_ref().and_then(|staged| staged.rules.deck().note()).unwrap_or_default().into());
    jailbreak::sync(ui, state, context, staged.as_ref());

    let jailed = state.borrow().jail.as_ref().is_some_and(|(_, board)| board.is_some());

    sandbox.set_playable(ready && (fielded || jailed) && staged.is_some() && state.borrow().battle.standing == Standing::default());

    match tab {
        Tab::Lineup => lineup::sync(ui, state, context, staged.as_ref()),
        Tab::Config => config::sync(ui, state, context, staged.as_ref()),
        Tab::Replay => replay::sync(ui, state, context),
        Tab::Stage => {}
    }

    battle::sync(ui, state);
}
