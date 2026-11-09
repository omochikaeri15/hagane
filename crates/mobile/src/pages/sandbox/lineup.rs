use std::cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, TimerMode, VecModel};

use foundry::domains::cat::combo::{self, CatCombo};
use foundry::domains::cat::filter::evaluation::entity_passes_filter;
use foundry::domains::cat::game::stats::{get_final_stats, seeded_level};
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::sandbox::jailbreak::Runs;
use foundry::domains::sandbox::orb::{self, Allowance, Kind};
use foundry::domains::sandbox::{BENCH_SLOTS, Cell, Edge, LINEUP_SLOTS, Lineup, Member, Placed, Roster, mount_of};
use foundry::systems::treasure::Bonus;

use crate::images::Images;
use crate::pages::{Context, cats};
use crate::roster::Entry;
use crate::{AppWindow, Cats, ComboRow, ComboSample, DeckCell, OrbSlot, Sandbox, SavedRow};

use super::orbs::{BEST_GRADE, Book};
use super::{Shared, Staging, jailbreak, refresh};

const NP_ICON: &str = "gatyaitemD_07_f.png";
const DOUBLE_TAP: Duration = Duration::from_millis(400);
const DROP_GUARD: Duration = Duration::from_millis(400);
const FIRST_TALENT_FORM: usize = 2;
const CONFIRM_MS: u64 = 2000;
pub(super) const CONFIRM_LABEL: &str = "Sure?";
pub(super) const DELETE_LABEL: &str = "Delete";
const EVERY_KIND: &str = "All Types";
const ANY_STRENGTH: &str = "Any Strength";
const BANNED_NOTE: &str = "Disabled on this stage";
const COMBO_SAMPLES: usize = 8;
const COMBO_TEXT_BLOCK: f32 = 135.0;
const COMBO_GLYPH_SHARE: f32 = 0.52;
const COMBO_NAME_SIZE: f32 = 16.0;
const COMBO_EFFECT_SIZE: f32 = 14.0;
const FIXED_STRENGTHS: usize = 3;

#[derive(Clone, Copy, Default)]
struct Priced {
    cost: i32,
    barred: bool,
    talented: bool,
}

pub(super) fn matches(cat: &CatEntry, query: &str) -> bool {
    cat.base_id_str().contains(query) || cat.names.iter().flatten().any(|name| name.to_lowercase().contains(query))
}

fn shown_form(cat: &CatEntry, banner_form: usize) -> usize {
    let highest = banner_form.min(cat.forms.len() - 1);

    (0..=highest)
        .rev()
        .find(|form| cat.forms[*form] && cat.banner_paths[*form].is_some())
        .or_else(|| (0..=highest).rev().find(|form| cat.forms[*form]))
        .or_else(|| cat.forms.iter().position(|exists| *exists))
        .unwrap_or(0)
}

fn price(lineup: &Lineup, cats: &[CatEntry], rules: Option<&Staging>) -> HashMap<Cell, Priced> {
    let mut priced = HashMap::new();
    let slots = lineup.slots.iter().enumerate().map(|(index, member)| (Cell::Slot(index), member));
    let bench = lineup.bench.iter().enumerate().filter_map(|(index, member)| Some((Cell::Bench(index), member.as_ref()?)));
    let mut fielded: Vec<usize> = Vec::new();

    for (cell, member) in slots.chain(bench) {
        let cat = cats.iter().find(|cat| cat.id == member.id);
        let stats = cat.and_then(|cat| cat.stats.get(member.form).and_then(Option::as_ref));
        let talents = (member.form >= FIRST_TALENT_FORM).then_some(&member.talents);
        let (level, plus) = member.levels(cat.and_then(|cat| cat.unitbuy.level_caps()));
        let base = stats.map_or(0, |stats| get_final_stats(stats, cat.and_then(|cat| cat.curve.as_ref()), (level + plus) as i32, cat.and_then(|cat| cat.talent_data.as_ref()), talents, &Bonus::default()).eoc1_cost);
        let rarity = cat.and_then(|cat| usize::try_from(cat.unitbuy.rarity).ok()).unwrap_or(0);
        let cost = rules.map_or(base, |staging| staging.rules.cost(base, rarity));
        let groups = cat.and_then(|cat| cat.talent_data.as_ref()).map_or(0, |talent| talent.groups.len());
        let talented = member.talented() && member.talents.iter().any(|(index, level)| *level > 0 && usize::from(*index) < groups);
        let earlier = fielded.iter().filter(|held| **held == rarity).count();
        let slot = match cell {
            Cell::Slot(slot) => Some(slot),
            Cell::Bench(_) => None,
        };
        let barred = rules.is_some_and(|staging| staging.rules.bars(member.id, rarity, cost, earlier, slot));

        priced.insert(cell, Priced { cost, talented, barred });

        if matches!(cell, Cell::Slot(_)) {
            fielded.push(rarity);
        }
    }

    priced
}

fn icon_of(images: &Images, context: &Rc<Context>, cats: &[CatEntry], id: u32, form: usize) -> Option<Image> {
    let vault = context.engine.vault();
    let path = match cats.iter().find(|cat| cat.id == id) {
        Some(cat) => scanner::deploy_icon(&vault.vfs, cat, form),
        None => scanner::orphan_icon(&vault.vfs, id, form),
    }?;

    images.trimmed(&vault, &path)
}

fn recruit(state: &Shared, context: &Rc<Context>, roster: &Roster, id: u32, form: Option<usize>) -> Option<Member> {
    let cat = state.borrow().cat(id)?;
    let settings = context.state.settings();
    let lowest = shown_form(&cat, settings.sandbox.banner_form);
    let level = seeded_level(&cat, &settings).1;
    let fresh = Member { id, form: form.unwrap_or(lowest), level, talents: HashMap::new(), orbs: Vec::new() };
    let vault = context.engine.vault();
    let mut member = roster.dress(fresh, form, &mount_of(&vault.vfs));

    member.orbs.resize(vault.vds.cats.orb_slots(&vault.vfs).get(&id).copied().unwrap_or(0), None);

    Some(member)
}

pub fn edit(state: &Shared, context: &Rc<Context>, change: impl FnOnce(&mut Roster)) {
    let mount = mount_of(&context.engine.vault().vfs);

    state.borrow().store.update(|saved| {
        change(&mut saved.roster);
        saved.roster.remember(&mount);
    });
}

pub fn fill_roster(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if jailbreak::editing(state) {
        return jailbreak::fill_roster(ui, state, context);
    }

    let Some(cats) = state.borrow().cats.clone() else {
        return;
    };
    let banner_form = context.state.settings().sandbox.banner_form;
    let entries: Vec<Entry> = cats
        .iter()
        .map(|cat| {
            let form = shown_form(cat, banner_form);

            Entry { id: cat.id, banner: cat.banner_paths.get(form).cloned().flatten().or_else(|| cat.image_path.clone()), tooltip: cat.display_name(form).into() }
        })
        .collect();
    let shown = shown_rows(state, context, &cats);

    state.borrow().roster.fill(context.engine.vault(), entries, shown.clone());
    ui.global::<Sandbox>().set_roster_count(shown.len() as i32);
}

fn shown_rows(state: &Shared, context: &Rc<Context>, cats: &[CatEntry]) -> Vec<usize> {
    let query = state.borrow().saved().search.to_lowercase();
    let pages = context.pages();
    let filter = pages.as_ref().map(|pages| pages.cats.borrow().filter.clone());

    cats.iter()
        .enumerate()
        .filter(|(_, cat)| filter.as_ref().is_none_or(|filter| entity_passes_filter(cat, filter)))
        .filter(|(_, cat)| query.is_empty() || matches(cat, &query))
        .map(|(index, _)| index)
        .collect()
}

pub fn refilter(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if jailbreak::editing(state) {
        return jailbreak::fill_roster(ui, state, context);
    }

    let Some(cats) = state.borrow().cats.clone() else {
        return;
    };
    let shown = shown_rows(state, context, &cats);

    state.borrow().roster.show(shown.clone());
    ui.global::<Sandbox>().set_roster_count(shown.len() as i32);
}

fn cell_at(index: i32, bench: bool) -> Cell {
    let index = index.max(0) as usize;

    if bench { Cell::Bench(index) } else { Cell::Slot(index) }
}

fn edge_of(after: bool) -> Edge {
    if after { Edge::After } else { Edge::Before }
}

pub(super) fn show_deck(ui: &AppWindow, state: &Shared) {
    let sandbox = ui.global::<Sandbox>();
    let held = state.borrow();
    let resting = |cell: Cell| {
        match cell {
            Cell::Slot(index) => held.resting.0.get(index),
            Cell::Bench(index) => held.resting.1.get(index),
        }
        .cloned()
        .unwrap_or_default()
    };
    let shown = |cell: Cell| match held.plan.as_ref().map(|plan| plan.get(cell)) {
        None => resting(cell),
        Some(Some(Placed::Kept(source))) => resting(source),
        Some(Some(Placed::Carried)) => DeckCell { aimed: true, ..DeckCell::default() },
        Some(None) => DeckCell::default(),
    };
    let slots: Vec<DeckCell> = (0..LINEUP_SLOTS).map(|index| shown(Cell::Slot(index))).collect();
    let bench: Vec<DeckCell> = (0..BENCH_SLOTS).map(|index| shown(Cell::Bench(index))).collect();

    sandbox.set_slots(ModelRc::new(VecModel::from(slots)));
    sandbox.set_bench(ModelRc::new(VecModel::from(bench)));
}

fn open_unit(ui: &AppWindow, state: &Shared, context: &Rc<Context>, cell: Cell, member: &Member) {
    let Some(cat) = state.borrow().cat(member.id) else {
        return;
    };
    let Some(pages) = context.pages() else {
        return;
    };

    cats::seed_member(ui, &pages.cats, context, cats::Seed { cat: &cat, form: member.form, level: &member.level, talents: &member.talents, locked: false, grants: None });

    let landing = Rc::clone(state);
    let ctx = Rc::clone(context);
    let id = member.id;

    cats::set_edited(&pages.cats, Some(Rc::new(move |ui: &AppWindow| sync_back(ui, &landing, &ctx, cell, id))));
    state.borrow_mut().editing = Some((cell, member.id));
    state.borrow_mut().orb_slot = None;
    ui.global::<Sandbox>().set_unit_open(true);
    refresh(ui, state, context);
}

fn sync_back(ui: &AppWindow, state: &Shared, context: &Rc<Context>, cell: Cell, id: u32) {
    let Some(pages) = context.pages() else {
        return;
    };
    let (form, level, talents) = {
        let cats_state = pages.cats.borrow();

        if cats_state.selected != Some(id) {
            return;
        }

        (cats_state.form, cats_state.level_input.clone(), cats_state.levels_for(id))
    };
    let current = state.borrow().saved().roster.current().and_then(|lineup| lineup.get(cell).cloned());
    let Some(current) = current.filter(|member| member.id == id) else {
        return;
    };

    if current.form == form && current.level == level && current.talents == talents {
        return;
    }

    edit(state, context, |roster| {
        if let Some(member) = roster.current_mut().get_mut(cell) {
            member.form = form;
            member.level = level;
            member.talents = talents;
        }
    });
    refresh(ui, state, context);
}

fn close_unit(state: &Shared, context: &Rc<Context>) {
    state.borrow_mut().editing = None;
    state.borrow_mut().orb_slot = None;

    if let Some(pages) = context.pages() {
        cats::set_edited(&pages.cats, None);
    }
}

fn editing_member(state: &Shared) -> Option<(Cell, Member)> {
    let held = state.borrow();
    let (cell, id) = held.editing?;
    let saved = held.saved();

    saved.roster.current().and_then(|lineup| lineup.get(cell).cloned()).filter(|member| member.id == id).map(|member| (cell, member))
}

fn set_orb(state: &Shared, context: &Rc<Context>, orb: Option<u32>) {
    let Some((cell, _)) = editing_member(state) else {
        return;
    };
    let Some(slot) = state.borrow().orb_slot else {
        return;
    };

    edit(state, context, |roster| {
        if let Some(member) = roster.current_mut().get_mut(cell)
            && let Some(target) = member.orbs.get_mut(slot)
        {
            *target = orb;
        }
    });
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = window.global::<Sandbox>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_recruit(move |id| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let id = id.max(0) as u32;

        if jailbreak::editing(&held) {
            jailbreak::recruit(&held, id);
            refresh(&ui, &held, &ctx);

            return;
        }

        edit(&held, &ctx, |roster| {
            if let Some(member) = recruit(&held, &ctx, roster, id, None) {
                roster.current_mut().add(member);
            }
        });
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_search_changed(move |text| {
        held.borrow().store.update(|saved| saved.search = text.to_string());

        if let Some(ui) = weak.upgrade() {
            refilter(&ui, &held, &ctx);
        }
    });

    let (_, ctx, weak) = on(state, context, window);
    sandbox.on_open_filter(move || {
        if let (Some(ui), Some(pages)) = (weak.upgrade(), ctx.pages()) {
            cats::open_filter(&ui, &pages.cats, &ctx);
        }
    });

    let (held, _, weak) = on(state, context, window);
    sandbox.on_cell_hovered(move |from, from_bench, to, to_bench, after| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if jailbreak::editing(&held) {
            if from >= 0 {
                jailbreak::hovered(&held, cell_at(from, from_bench), (to >= 0).then(|| cell_at(to, to_bench)), edge_of(after));
            }

            show_deck(&ui, &held);

            return;
        }

        let plan = if from >= 0 && to >= 0 {
            held.borrow().saved().roster.current().and_then(|lineup| lineup.arrange(Some(cell_at(from, from_bench)), cell_at(to, to_bench), edge_of(after)))
        } else {
            None
        };

        held.borrow_mut().plan = plan;
        show_deck(&ui, &held);
    });

    let last_drop = Rc::new(cell::Cell::new(None::<Instant>));
    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_cell_held(move |index, bench| {
        if !jailbreak::editing(&held) {
            return;
        }

        jailbreak::held_down(&held, cell_at(index, bench));

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    let dropped_at = Rc::clone(&last_drop);
    sandbox.on_cell_tapped(move |index, bench| {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if dropped_at.get().is_some_and(|at| at.elapsed() < DROP_GUARD) {
            return;
        }
        let cell = cell_at(index, bench);

        if jailbreak::editing(&held) {
            jailbreak::tapped(&ui, &held, &ctx, cell);
            refresh(&ui, &held, &ctx);

            return;
        }

        let member = held.borrow().saved().roster.current().and_then(|lineup| lineup.get(cell).cloned());

        if let Some(member) = member {
            open_unit(&ui, &held, &ctx, cell, &member);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    let dropped_at = Rc::clone(&last_drop);
    sandbox.on_cell_dropped(move |from, from_bench, to, to_bench, after| {
        let (from, to) = (cell_at(from, from_bench), cell_at(to, to_bench));

        dropped_at.set(Some(Instant::now()));
        held.borrow_mut().plan = None;

        if jailbreak::editing(&held) {
            jailbreak::dropped(&held, from, to, edge_of(after));
        } else if from != to {
            edit(&held, &ctx, |roster| {
                let lineup = roster.current_mut();

                if let Some(plan) = lineup.arrange(Some(from), to, edge_of(after)) {
                    lineup.commit(&plan, None);
                }
            });
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    let dropped_at = Rc::clone(&last_drop);
    sandbox.on_cell_discarded(move |index, bench| {
        dropped_at.set(Some(Instant::now()));
        held.borrow_mut().plan = None;

        if jailbreak::editing(&held) {
            jailbreak::discarded(&held, cell_at(index, bench));
        } else {
            edit(&held, &ctx, |roster| {
                roster.current_mut().take(cell_at(index, bench));
            });
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let last_tap = Rc::new(cell::Cell::new(None::<(i32, Instant)>));
    let weak = window.as_weak();
    sandbox.on_combo_tapped(move |line| {
        let now = Instant::now();
        let doubled = last_tap.get().is_some_and(|(held, at)| held == line && now.duration_since(at) <= DOUBLE_TAP);
        last_tap.set((!doubled).then_some((line, now)));

        if doubled && let Some(ui) = weak.upgrade() {
            ui.global::<Sandbox>().invoke_adopt_combo(line);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_combo_search_changed(move |text| {
        held.borrow_mut().combos.query = text.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_combo_kind_set(move |index| {
        held.borrow_mut().combos.kind = index;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_combo_strength_set(move |index| {
        held.borrow_mut().combos.strength = index;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_adopt_combo(move |line| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let vault = ctx.engine.vault();
        let rows = vault.vds.cats.combos(&vault.vfs);
        let Some(row) = usize::try_from(line).ok().and_then(|line| rows.get(line)) else {
            return;
        };

        edit(&held, &ctx, |roster| {
            let mut lineup = std::mem::take(roster.current_mut());

            lineup.adopt(row, |id, form| {
                recruit(&held, &ctx, roster, id, Some(form)).unwrap_or_else(|| {
                    let mut member = Member { id, form, ..Member::default() };

                    member.orbs.resize(vault.vds.cats.orb_slots(&vault.vfs).get(&id).copied().unwrap_or(0), None);
                    member
                })
            });
            *roster.current_mut() = lineup;
        });
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_lineup_new(move || {
        if jailbreak::editing(&held) {
            jailbreak::shelve(&held, Runs::create);
        } else {
            edit(&held, &ctx, Roster::create);
        }

        held.borrow_mut().deleting = false;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_lineup_delete(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        if !held.borrow().deleting {
            held.borrow_mut().deleting = true;

            let landing = Rc::clone(&held);
            let ctx_inner = Rc::clone(&ctx);
            let weak_inner = ui.as_weak();

            held.borrow().delete_timer.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                landing.borrow_mut().deleting = false;

                if let Some(ui) = weak_inner.upgrade() {
                    refresh(&ui, &landing, &ctx_inner);
                }
            });
            refresh(&ui, &held, &ctx);

            return;
        }

        held.borrow_mut().deleting = false;

        if jailbreak::editing(&held) {
            jailbreak::shelve(&held, Runs::delete);
        } else {
            edit(&held, &ctx, Roster::delete);
        }

        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_lineup_pick(move |index| {
        let index = index.max(0) as usize;

        if jailbreak::editing(&held) {
            jailbreak::shelve(&held, |runs| runs.load(index));
        } else {
            edit(&held, &ctx, |roster| {
                if index < roster.lineups.len() {
                    roster.selected = index;
                }
            });
        }

        held.borrow_mut().deleting = false;

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_lineup_renamed(move |name| {
        if jailbreak::editing(&held) {
            jailbreak::shelve(&held, |runs| runs.current_mut().name = name.to_string());
        } else {
            edit(&held, &ctx, |roster| roster.current_mut().name = name.to_string());
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_saved_search_changed(move |text| {
        held.borrow_mut().saved_search = text.to_string();

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_unit_closed(move || {
        close_unit(&held, &ctx);
        held.borrow_mut().jail_viewing = None;

        if let Some(ui) = weak.upgrade() {
            ui.global::<Sandbox>().set_orb_open(false);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    window.global::<Cats>().on_orb_tapped(move |slot| {
        held.borrow_mut().orb_slot = Some(slot.max(0) as usize);

        if let Some(ui) = weak.upgrade() {
            ui.global::<Sandbox>().set_orb_open(true);
            refresh(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_orb_type_set(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(book) = super::book(&held, &ctx) else {
            return;
        };
        let Some((_, member)) = editing_member(&held) else {
            return;
        };
        let Some(cat) = held.borrow().cat(member.id) else {
            return;
        };
        let allowance = Allowance::of(&cat);
        let offered = book.offered(allowance);
        let Some(content) = usize::try_from(index).ok().and_then(|index| offered.get(index)).copied() else {
            return;
        };
        let current = held.borrow().orb_slot.and_then(|slot| member.orbs.get(slot).copied().flatten());
        let grade = current.and_then(|orb| orb::grade(&book.orbs, orb)).unwrap_or(BEST_GRADE);
        let aimed = current.and_then(|orb| orb::kind(&book.orbs, orb)).and_then(|kind| kind.attribute);
        let aims = book.aims(content, allowance);
        let attribute = aims.iter().find(|attribute| Some(**attribute) == aimed).or_else(|| aims.first()).copied();

        set_orb(&held, &ctx, orb::pick(&book.orbs, Kind { content, attribute }, grade));
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_orb_trait_set(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(book) = super::book(&held, &ctx) else {
            return;
        };
        let Some((_, member)) = editing_member(&held) else {
            return;
        };
        let Some(cat) = held.borrow().cat(member.id) else {
            return;
        };
        let current = held.borrow().orb_slot.and_then(|slot| member.orbs.get(slot).copied().flatten());
        let Some(kind) = current.and_then(|orb| orb::kind(&book.orbs, orb)) else {
            return;
        };
        let aims = book.aims(kind.content, Allowance::of(&cat));
        let Some(attribute) = usize::try_from(index).ok().and_then(|index| aims.get(index)).copied() else {
            return;
        };
        let grade = current.and_then(|orb| orb::grade(&book.orbs, orb)).unwrap_or(BEST_GRADE);

        set_orb(&held, &ctx, orb::pick(&book.orbs, Kind { attribute: Some(attribute), ..kind }, grade));
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_orb_grade_set(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let Some(book) = super::book(&held, &ctx) else {
            return;
        };
        let Some((_, member)) = editing_member(&held) else {
            return;
        };
        let current = held.borrow().orb_slot.and_then(|slot| member.orbs.get(slot).copied().flatten());
        let Some(kind) = current.and_then(|orb| orb::kind(&book.orbs, orb)) else {
            return;
        };
        let grades = book.grades(kind);
        let Some(grade) = usize::try_from(index).ok().and_then(|index| grades.get(index)).copied() else {
            return;
        };

        set_orb(&held, &ctx, orb::pick(&book.orbs, kind, grade));
        refresh(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = on(state, context, window);
    sandbox.on_orb_cleared(move || {
        set_orb(&held, &ctx, None);

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held, &ctx);
        }
    });
}

struct Painter<'a> {
    images: &'a Images,
    context: &'a Rc<Context>,
    cats: &'a [CatEntry],
    book: Option<&'a Rc<Book>>,
    altar: Option<u32>,
}

fn cell_of(painter: &Painter<'_>, member: Option<&Member>, priced: Priced) -> DeckCell {
    let Painter { images, context, cats, book, altar } = *painter;
    let Some(member) = member else {
        return DeckCell::default();
    };
    let vault = context.engine.vault();
    let icon = icon_of(images, context, cats, member.id, member.form);
    let caps = cats.iter().find(|cat| cat.id == member.id).and_then(|cat| cat.unitbuy.level_caps());
    let (level, plus) = match (member.levels(caps), altar) {
        ((level, _), Some(cap)) => (level.min(cap.saturating_add(1)), 0),
        (levels, None) => levels,
    };
    let slots = vault.vds.cats.orb_slots(&vault.vfs).get(&member.id).copied().unwrap_or(0);
    let orbs: Vec<Image> = book
        .map(|book| member.worn(slots, &book.orbs).filter_map(|(_, orb)| book.face(images, &vault, orb)).collect())
        .unwrap_or_default();
    let np = vault.vfs.find(NP_ICON).and_then(|path| images.trimmed(&vault, &path));

    DeckCell {
        filled: true,
        registered: false,
        aimed: false,
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
        fallback: format!("{:03}-{}", member.id, member.form + 1).into(),
        level: if plus > 0 { format!("Lv{level}+{plus}") } else { format!("Lv{level}") }.into(),
        cost: format!("{}\u{00a2}", priced.cost).into(),
        has_cost: true,
        barred: priced.barred,
        talented: priced.talented,
        has_np: np.is_some(),
        np: np.unwrap_or_default(),
        orbs: ModelRc::new(VecModel::from(orbs)),
    }
}

#[derive(Default)]
pub struct ComboFilters {
    pub query: String,
    pub kind: i32,
    pub strength: i32,
}

#[derive(Clone, Copy)]
enum Strength {
    Any,
    Active,
    Inactive,
    Band(i32),
}

fn estimated_lines(text: &str, size: f32) -> usize {
    let glyphs: usize = text.chars().map(|glyph| if glyph.is_ascii() { 1 } else { 2 }).sum();

    (glyphs as f32 * size * COMBO_GLYPH_SHARE / COMBO_TEXT_BLOCK).ceil() as usize
}

fn tallest_looking(combos: &[(&CatCombo, Option<&str>)]) -> Vec<ComboSample> {
    let weight = |combo: &CatCombo| {
        let lines = estimated_lines(&combo.name, COMBO_NAME_SIZE) + estimated_lines(&combo.effect, COMBO_EFFECT_SIZE);

        (lines, combo.name.chars().count() + combo.effect.chars().count())
    };
    let mut noted: Vec<_> = combos.iter().filter(|(_, note)| note.is_some()).collect();
    let mut plain: Vec<_> = combos.iter().filter(|(_, note)| note.is_none()).collect();

    noted.sort_by_key(|(combo, _)| std::cmp::Reverse(weight(combo)));
    plain.sort_by_key(|(combo, _)| std::cmp::Reverse(weight(combo)));

    noted
        .into_iter()
        .take(COMBO_SAMPLES / 2)
        .chain(plain.into_iter().take(COMBO_SAMPLES / 2))
        .map(|(combo, note)| ComboSample { name: combo.name.as_str().into(), effect: combo.effect.as_str().into(), note: note.unwrap_or_default().into() })
        .collect()
}

pub(super) fn sync_combos(ui: &AppWindow, context: &Rc<Context>, cats: &[CatEntry], lineup: &Lineup, staging: Option<&Staging>, focus: Option<(u32, usize)>, filters: &ComboFilters) {
    let sandbox = ui.global::<Sandbox>();
    let images = &context.images;
    let Some(globals) = context.globals() else {
        return;
    };
    let vault = context.engine.vault();
    let rows = vault.vds.cats.combos(&vault.vfs);
    let unlocked = lineup.unlocked(&vault.vds.cats.talents(&vault.vfs));
    let every = combo::every(globals.context(&vault), cats);
    let mut kinds: Vec<(i32, SharedString)> = Vec::new();
    let mut bands: Vec<(i32, SharedString)> = Vec::new();

    for combo in &every {
        if !kinds.iter().any(|(effect, _)| *effect == combo.effect_type) {
            kinds.push((combo.effect_type, combo.kind.as_str().into()));
        }

        if !combo.band.is_empty() && !bands.iter().any(|(level, _)| *level == combo.effect_level) {
            bands.push((combo.effect_level, combo.band.as_str().into()));
        }
    }

    kinds.sort_by_key(|(effect, _)| *effect);
    bands.sort_by_key(|(level, _)| *level);

    let kind_index = usize::try_from(filters.kind).ok().filter(|index| *index <= kinds.len()).unwrap_or(0);
    let strength_index = usize::try_from(filters.strength).ok().filter(|index| *index < FIXED_STRENGTHS + bands.len()).unwrap_or(0);
    let wanted_kind = kind_index.checked_sub(1).map(|index| kinds[index].0);
    let strength = match strength_index {
        0 => Strength::Any,
        1 => Strength::Active,
        2 => Strength::Inactive,
        index => Strength::Band(bands[index - FIXED_STRENGTHS].0),
    };
    let query = filters.query.trim().to_lowercase();
    let noted: Vec<(&CatCombo, Option<&str>)> = every
        .iter()
        .map(|combo| (combo, if staging.is_some_and(|staging| staging.rules.bans(combo.effect_type)) { Some(BANNED_NOTE) } else { combo.restriction.as_deref() }))
        .collect();
    let samples = tallest_looking(&noted);

    drop(noted);

    if sandbox.get_combo_samples().iter().ne(samples.iter().cloned()) {
        sandbox.set_combo_samples(ModelRc::new(VecModel::from(samples)));
    }

    let mut cards: Vec<(CatCombo, bool, bool)> = every
        .into_iter()
        .map(|combo| {
            let banned = staging.is_some_and(|staging| staging.rules.bans(combo.effect_type));
            let active = !banned && rows.get(combo.line).is_some_and(|row| lineup.powers(combo.line, row, &unlocked));

            (combo, active, banned)
        })
        .filter(|(combo, active, _)| {
            let strong = match strength {
                Strength::Any => true,
                Strength::Active => *active,
                Strength::Inactive => !*active,
                Strength::Band(level) => combo.effect_level == level,
            };
            let joined = focus.is_none_or(|(unit, form)| combo.members.iter().any(|member| member.id == Some(unit) && member.form <= form));

            joined && strong && wanted_kind.is_none_or(|effect| combo.effect_type == effect) && combo.name.to_lowercase().contains(&query)
        })
        .collect();

    cards.sort_by(|(left, left_active, left_banned), (right, right_active, right_banned)| {
        right_active
            .cmp(left_active)
            .then_with(|| left_banned.cmp(right_banned))
            .then_with(|| left.effect_type.cmp(&right.effect_type))
            .then_with(|| right.effect_level.cmp(&left.effect_level))
            .then_with(|| left.name.cmp(&right.name))
    });

    let shown: Vec<ComboRow> = cards
        .into_iter()
        .map(|(combo, active, banned)| {
            let note = if banned { Some(BANNED_NOTE.to_owned()) } else { combo.restriction.clone() };
            let icons: Vec<Image> = combo
                .members
                .iter()
                .map(|member| {
                    member
                        .icon
                        .as_ref()
                        .and_then(|source| images.trimmed(&vault, &source.path))
                        .or_else(|| member.id.and_then(|id| icon_of(images, context, cats, id, member.form)))
                        .unwrap_or_default()
                })
                .collect();
            let has: Vec<bool> = icons.iter().map(|icon| Images::size_of(icon).0 > 0).collect();

            ComboRow {
                line: combo.line as i32,
                name: combo.name.as_str().into(),
                effect: combo.effect.as_str().into(),
                note: note.unwrap_or_default().into(),
                active,
                banned,
                icons: ModelRc::new(VecModel::from(icons)),
                has: ModelRc::new(VecModel::from(has)),
            }
        })
        .collect();
    let kind_labels: Vec<SharedString> = std::iter::once(EVERY_KIND.into()).chain(kinds.into_iter().map(|(_, label)| label)).collect();
    let strength_labels: Vec<SharedString> = [ANY_STRENGTH, "Active", "Inactive"].into_iter().map(SharedString::from).chain(bands.into_iter().map(|(_, label)| label)).collect();

    sandbox.set_combo_kinds(ModelRc::new(VecModel::from(kind_labels)));
    sandbox.set_combo_kind(kind_index as i32);
    sandbox.set_combo_strengths(ModelRc::new(VecModel::from(strength_labels)));
    sandbox.set_combo_strength(strength_index as i32);
    sandbox.set_combo_search(filters.query.as_str().into());
    sandbox.set_combos(ModelRc::new(VecModel::from(shown)));
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>, staging: Option<&Staging>) {
    if jailbreak::editing(state) {
        return jailbreak::sync_tab(ui, state, context, staging);
    }

    let sandbox = ui.global::<Sandbox>();
    let Some(cats) = state.borrow().cats.clone() else {
        return;
    };
    let book = super::book(state, context);
    let images = &context.images;
    let (lineup, editing, orb_slot, deleting, saved_search, names, selected_lineup) = {
        let held = state.borrow();
        let saved = held.saved();
        let names: Vec<(usize, String, usize)> = saved.roster.lineups.iter().enumerate().map(|(index, lineup)| (index, lineup.name.clone(), lineup.slots.len())).collect();

        (saved.roster.current().cloned().unwrap_or_default(), held.editing, held.orb_slot, held.deleting, held.saved_search.clone(), names, saved.roster.selected)
    };
    let priced = price(&lineup, &cats, staging);
    let painter = Painter { images, context, cats: &cats, book: book.as_ref(), altar: staging.and_then(|staging| staging.altar) };
    let slots: Vec<DeckCell> = (0..LINEUP_SLOTS)
        .map(|index| cell_of(&painter, lineup.slots.get(index), priced.get(&Cell::Slot(index)).copied().unwrap_or_default()))
        .collect();
    let bench: Vec<DeckCell> = (0..BENCH_SLOTS)
        .map(|index| cell_of(&painter, lineup.bench.get(index).and_then(Option::as_ref), priced.get(&Cell::Bench(index)).copied().unwrap_or_default()))
        .collect();

    state.borrow_mut().resting = (slots, bench);
    show_deck(ui, state);
    sandbox.set_lineup_name(lineup.name.as_str().into());
    sandbox.set_search(state.borrow().saved().search.as_str().into());
    sandbox.set_filter_active(context.pages().is_some_and(|pages| pages.cats.borrow().filter.is_active()));

    let focus = editing.and_then(|(cell, id)| lineup.get(cell).filter(|member| member.id == id).map(|member| (id, member.form)));

    sync_combos(ui, context, &cats, &lineup, staging, focus, &state.borrow().combos);

    let wanted = saved_search.to_lowercase();
    let saved_rows: Vec<SavedRow> = names
        .into_iter()
        .filter(|(_, name, _)| wanted.is_empty() || name.to_lowercase().contains(&wanted))
        .map(|(index, name, count)| SavedRow { index: index as i32, name: name.into(), count: format!("{count}/{LINEUP_SLOTS}").into(), selected: index == selected_lineup })
        .collect();

    sandbox.set_saved(ModelRc::new(VecModel::from(saved_rows)));
    sandbox.set_delete_label(if deleting { CONFIRM_LABEL.into() } else { DELETE_LABEL.into() });

    let edited = editing.and_then(|(cell, id)| lineup.get(cell).filter(|member| member.id == id).cloned());

    match (&edited, &book) {
        (Some(member), Some(book)) => {
            let vault = context.engine.vault();
            let cat = cats.iter().find(|cat| cat.id == member.id);

            sandbox.set_unit_title(cat.map_or_else(|| format!("Unit {:03}", member.id), |cat| cat.display_name(member.form)).into());
            ui.global::<Cats>().set_has_orbs(member.form >= FIRST_TALENT_FORM && !member.orbs.is_empty());
            ui.global::<Cats>().set_orb_slots(ModelRc::new(VecModel::from(member.orbs.iter().map(|orb| book.picture(images, &vault, *orb)).collect::<Vec<OrbSlot>>())));

            let held_orb = orb_slot.and_then(|slot| member.orbs.get(slot).copied().flatten());
            let allowance = cat.map(Allowance::of).unwrap_or_default();
            let kind = held_orb.and_then(|orb| orb::kind(&book.orbs, orb));
            let offered = book.offered(allowance);
            let chosen = kind.map(|kind| kind.content).and_then(|content| offered.iter().position(|held| *held == content));
            let aims = kind.map_or_else(Vec::new, |kind| book.aims(kind.content, allowance));
            let aim = kind.and_then(|kind| kind.attribute).and_then(|attribute| aims.iter().position(|held| *held == attribute));
            let grades = kind.map_or_else(Vec::new, |kind| book.grades(kind));
            let grade = held_orb.and_then(|orb| orb::grade(&book.orbs, orb)).and_then(|grade| grades.iter().position(|held| *held == grade));

            sandbox.set_orb_preview(book.picture(images, &vault, held_orb));
            sandbox.set_orb_types(ModelRc::new(VecModel::from(offered.iter().map(|content| book.effect_label(*content).into()).collect::<Vec<slint::SharedString>>())));
            sandbox.set_orb_type(chosen.map_or(-1, |index| index as i32));
            sandbox.set_orb_traits(ModelRc::new(VecModel::from(aims.iter().map(|attribute| book.trait_label(*attribute).into()).collect::<Vec<slint::SharedString>>())));
            sandbox.set_orb_trait(aim.map_or(-1, |index| index as i32));
            sandbox.set_orb_trait_enabled(!aims.is_empty());
            sandbox.set_orb_grades(ModelRc::new(VecModel::from(grades.iter().map(|grade| Book::grade_label(*grade).into()).collect::<Vec<slint::SharedString>>())));
            sandbox.set_orb_grade(grade.map_or(-1, |index| index as i32));
            sandbox.set_orb_held(held_orb.is_some());
        }
        _ => {
            ui.global::<Cats>().set_has_orbs(false);
        }
    }
}
