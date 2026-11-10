use std::collections::HashMap;
use std::rc::Rc;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use emulator::runtime::{RoguelikePicks, RosterPick, SetupUnit};
use foundry::domains::cat::filter::evaluation::entity_passes_filter;
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::sandbox::jailbreak::{Board, Grants, Run, Runs, is_challenge_map};
use foundry::domains::sandbox::{BENCH_SLOTS, Cell, Edge, LINEUP_SLOTS, Member};
use foundry::systems::combat::registry::AbilityIcon;

use crate::pages::{Context, cats};
use crate::roster::Entry;
use crate::{AppWindow, Cats, DeckCell, JailBonus, Sandbox, SavedRow};

use super::state::{Saved, Tab};
use super::{Shared, Staging, lineup, refresh};

pub type Key = (i32, i32, u64, usize);

fn roamed(state: &Shared, context: &Rc<Context>) -> Option<(i32, i32)> {
    let (map, crown) = context.pages()?.stages.borrow().roamed()?;
    let global = i32::try_from(state.borrow().bundle.as_ref()?.registry.addresses.get(&map)?.global?).ok()?;

    is_challenge_map(&context.engine.vault().vfs, global).then_some((global, i32::from(crown)))
}

pub fn board(state: &Shared, context: &Rc<Context>, staging: Option<&Staging>) -> Option<Rc<Board>> {
    let cats = state.borrow().cats.clone();
    let vault = context.engine.vault();
    let jail = staging.map_or_else(|| roamed(state, context), |staging| staging.jail);
    let key: Option<Key> = jail.map(|(map_id, crown)| (map_id, crown, vault.vfs.fingerprint(), cats.as_ref().map_or(0, |cats| cats.len())));

    if let Some((held, board)) = state.borrow().jail.as_ref()
        && Some(*held) == key
    {
        return board.clone();
    }

    let loaded = key.zip(cats).zip(context.globals()).and_then(|(((map_id, crown, ..), cats), globals)| Board::load(globals.context(&vault), &cats, map_id, crown)).map(Rc::new);

    state.borrow_mut().jail = key.map(|key| (key, loaded.clone()));

    loaded
}

fn held(state: &Shared) -> Option<Rc<Board>> {
    state.borrow().jail.as_ref().and_then(|(_, board)| board.clone())
}

pub fn run(saved: &Saved, board: &Board) -> Run {
    saved.config.jailbreak.get(&board.map_id).and_then(Runs::current).map_or_else(Run::default, |run| board.settled(run))
}

fn stage_of(context: &Rc<Context>) -> i32 {
    context.pages().and_then(|pages| pages.stages.borrow().picked()).and_then(|(id, _)| i32::try_from(id.stage).ok()).unwrap_or(0)
}

pub fn within_budget(state: &Shared, context: &Rc<Context>) -> bool {
    held(state).is_none_or(|board| board.within_budget(&run(&state.borrow().saved(), &board), stage_of(context)))
}

pub fn picks(saved: &Saved, board: &Board) -> (RoguelikePicks, Vec<SetupUnit>) {
    let run = run(saved, board);
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

    (RoguelikePicks { roster, buffs: run.bonuses.into_iter().collect(), cannon, registered }, fielded)
}

fn edit(state: &Shared, board: &Board, change: impl FnOnce(&Board, &mut Run)) {
    state.borrow().store.update(|saved| {
        let run = saved.config.jailbreak.entry(board.map_id).or_default().current_mut();

        board.settle(run);
        change(board, run);
    });
}

pub fn shelve(state: &Shared, change: impl FnOnce(&mut Runs)) {
    let Some(board) = held(state) else {
        return;
    };

    state.borrow().store.update(|saved| change(saved.config.jailbreak.entry(board.map_id).or_default()));
    state.borrow_mut().jail_viewing = None;
    state.borrow_mut().jail_shown = None;
}

pub fn editing(state: &Shared) -> bool {
    state.borrow().jail_lineup && held(state).is_some()
}

fn listed(board: &Board, run: &Run, cats: &[CatEntry]) -> Vec<(usize, usize)> {
    cats.iter().enumerate().filter(|(_, cat)| board.cat(cat.id).is_some()).map(|(index, cat)| (index, board.standing(run, cat.id).form)).collect()
}

pub fn fill_roster(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let (Some(board), Some(cats)) = (held(state), state.borrow().cats.clone()) else {
        return;
    };
    let run = run(&state.borrow().saved(), &board);
    let query = state.borrow().saved().search.to_lowercase();
    let all = listed(&board, &run, &cats);
    let forms: HashMap<usize, usize> = all.iter().copied().collect();
    let entries: Vec<Entry> = cats
        .iter()
        .enumerate()
        .map(|(index, cat)| {
            let form = forms.get(&index).copied().unwrap_or(0);
            let banner = (0..=form.min(cat.banner_paths.len().saturating_sub(1))).rev().find_map(|form| cat.banner_paths.get(form).cloned().flatten());

            Entry { id: cat.id, banner: banner.or_else(|| cat.image_path.clone()), tooltip: cat.display_name(form).into() }
        })
        .collect();
    let rows: Vec<usize> = {
        let held = state.borrow();

        all.iter()
            .map(|(index, _)| *index)
            .filter(|index| cats.get(*index).is_some_and(|cat| entity_passes_filter(cat, &held.filter) && (query.is_empty() || lineup::matches(cat, &query))))
            .collect()
    };

    state.borrow_mut().jail_listed = all;
    state.borrow().roster.fill(context.engine.vault(), entries, rows.clone());
    ui.global::<Sandbox>().set_roster_count(rows.len() as i32);
}

fn cell_of(context: &Rc<Context>, cats: &[CatEntry], run: &Run, member: Option<&Member>, flagged: bool) -> DeckCell {
    let Some(member) = member else {
        return DeckCell::default();
    };
    let vault = context.engine.vault();
    let cat = cats.iter().find(|cat| cat.id == member.id);
    let path = match cat {
        Some(cat) => scanner::deploy_icon(&vault.vfs, cat, member.form),
        None => scanner::orphan_icon(&vault.vfs, member.id, member.form),
    };
    let icon = path.and_then(|path| context.images.trimmed(&vault, &path));
    let (level, plus) = member.levels(None);

    DeckCell {
        filled: true,
        flagged,
        registered: run.registered.contains(&member.id),
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
        fallback: format!("{:03}-{}", member.id, member.form + 1).into(),
        level: if plus > 0 { format!("Lv{level}+{plus}") } else { format!("Lv{level}") }.into(),
        ..DeckCell::default()
    }
}

pub fn sync_deck(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let (Some(board), Some(cats)) = (held(state), state.borrow().cats.clone()) else {
        return;
    };
    let run = run(&state.borrow().saved(), &board);
    let lineup = board.lineup(&run);
    let recruits_over = board.tally(&run, stage_of(context)).is_some_and(|tally| tally.recruits_over);
    let slots: Vec<DeckCell> = (0..LINEUP_SLOTS)
        .map(|index| {
            let member = lineup.slots.get(index);
            let flagged = recruits_over && member.is_some_and(|member| board.recruited(member.id));

            cell_of(context, &cats, &run, member, flagged)
        })
        .collect();
    let bench: Vec<DeckCell> = (0..BENCH_SLOTS).map(|index| cell_of(context, &cats, &run, lineup.bench.get(index).and_then(Option::as_ref), false)).collect();
    let sandbox = ui.global::<Sandbox>();

    state.borrow_mut().resting = (slots, bench);
    lineup::show_deck(ui, state);
    sandbox.set_search(state.borrow().saved().search.as_str().into());
    sandbox.set_filter_active(state.borrow().filter.is_active());

    let viewed = state.borrow().jail_viewing.and_then(|unit| lineup.find(unit)).and_then(|cell| lineup.get(cell).cloned());

    match viewed {
        Some(member) => {
            sandbox.set_unit_title(cats.iter().find(|cat| cat.id == member.id).map_or_else(|| format!("Unit {:03}", member.id), |cat| cat.display_name(member.form)).into());
            show(ui, state, context, &member, board.grants(&run, member.id));
        }
        None if state.borrow().jail_viewing.is_some() => {
            state.borrow_mut().jail_viewing = None;
            state.borrow_mut().jail_shown = None;
            sandbox.set_unit_open(false);
        }
        None => {}
    }
}

fn show(ui: &AppWindow, state: &Shared, context: &Rc<Context>, member: &Member, grants: Grants) {
    let unchanged = state.borrow().jail_shown.as_ref().is_some_and(|(shown, held)| shown == member && **held == grants);
    let (Some(cat), Some(pages)) = (state.borrow().cat(member.id), context.pages()) else {
        return;
    };

    if unchanged && cats::lent(&pages.cats, member.id) {
        return;
    }

    let grants = Rc::new(grants);

    state.borrow_mut().jail_shown = Some((member.clone(), Rc::clone(&grants)));
    cats::seed_member(ui, &pages.cats, context, cats::Seed { cat: &cat, form: member.form, level: &member.level, talents: &member.talents, locked: true, grants: Some(grants) });
}

fn bonuses(context: &Rc<Context>, board: &Board, run: &Run, query: &str, global: bool) -> Vec<JailBonus> {
    let vault = context.engine.vault();
    let wanted = query.trim().to_lowercase();
    let mut shown: Vec<(String, JailBonus)> = Vec::new();

    for bonus in &board.bonuses {
        if bonus.target.is_none() != global || !board.offers(run, bonus) || !(wanted.is_empty() || board.label(run, bonus).to_lowercase().contains(&wanted)) {
            continue;
        }

        let title = board.title(run, bonus);
        let icon = match bonus.icon {
            AbilityIcon::Standard(cut) => context.images.cut(&vault, bonus.sheet, cut),
            AbilityIcon::Custom(_) | AbilityIcon::None => None,
        };

        shown.push((
            title,
            JailBonus {
                id: bonus.id,
                title: Default::default(),
                detail: board.detail(run, bonus).into(),
                has_icon: icon.is_some(),
                icon: icon.unwrap_or_default(),
                fallback: bonus.fallback.into(),
                level: run.level(bonus.id),
                limit: bonus.limit,
                permanent: bonus.permanent,
            },
        ));
    }

    let order: Vec<String> = shown.iter().fold(Vec::new(), |mut seen, (title, _)| {
        if !seen.contains(title) {
            seen.push(title.clone());
        }

        seen
    });

    shown.sort_by_key(|(title, _)| order.iter().position(|held| held == title));

    let mut last = String::new();

    shown
        .into_iter()
        .map(|(title, mut row)| {
            if title != last {
                row.title = title.as_str().into();
                last = title;
            }

            row
        })
        .collect()
}

pub fn sync(ui: &AppWindow, state: &Shared, context: &Rc<Context>, staging: Option<&Staging>) {
    let sandbox = ui.global::<Sandbox>();
    let board = board(state, context, staging);
    let (on_tab, was_editing, global) = {
        let held = state.borrow();

        (held.saved().tab == Tab::Lineup, held.jail_lineup, held.jail_global)
    };
    let editing = board.is_some() && on_tab;
    let bonuses_open = editing && sandbox.get_jail_bonuses_open();

    sandbox.set_jail_ready(board.is_some());
    sandbox.set_jail_bonuses_open(bonuses_open);
    sandbox.set_jail_global(global);
    state.borrow_mut().jail_lineup = editing;

    if editing != was_editing {
        state.borrow_mut().jail_viewing = None;
        state.borrow_mut().jail_shown = None;
        state.borrow_mut().plan = None;
        sandbox.set_unit_open(false);
        sandbox.set_unit_locked(false);
        lineup::fill_roster(ui, state, context);
    }

    let Some(board) = board else {
        sandbox.set_jail_tallied(false);

        return;
    };
    let run = run(&state.borrow().saved(), &board);
    let tally = board.tally(&run, stage_of(context));

    sandbox.set_jail_tallied(tally.is_some());
    sandbox.set_jail_recruits_over(tally.as_ref().is_some_and(|tally| tally.recruits_over));
    sandbox.set_jail_bonuses_over(tally.as_ref().is_some_and(|tally| tally.bonuses_over));
    sandbox.set_jail_recruits(tally.as_ref().map_or_else(Default::default, |tally| tally.recruits_note().unwrap_or(tally.recruits.as_str()).into()));
    sandbox.set_jail_bonuses_text(tally.as_ref().map_or_else(Default::default, |tally| tally.bonuses_note().unwrap_or(tally.bonuses.as_str()).into()));

    if editing && editing == was_editing {
        let stale = state.borrow().cats.as_ref().is_none_or(|cats| listed(&board, &run, cats) != state.borrow().jail_listed);

        if stale {
            fill_roster(ui, state, context);
        }
    }

    if bonuses_open {
        let query = sandbox.get_jail_search().to_string();
        let rows = bonuses(context, &board, &run, &query, global);
        let shown = state.borrow().jail_rows.clone();
        let same = shown.as_ref().is_some_and(|held| held.row_count() == rows.len() && held.iter().zip(&rows).all(|(old, new)| old.id == new.id));

        match shown.filter(|_| same) {
            Some(held) => {
                for (index, row) in rows.into_iter().enumerate() {
                    if held.row_data(index).as_ref() != Some(&row) {
                        held.set_row_data(index, row);
                    }
                }
            }
            None => {
                let held = Rc::new(VecModel::from(rows));

                sandbox.set_jail_bonuses(ModelRc::from(Rc::clone(&held)));
                state.borrow_mut().jail_rows = Some(held);
            }
        }
    } else {
        state.borrow_mut().jail_rows = None;
    }
}

pub fn sync_tab(ui: &AppWindow, state: &Shared, context: &Rc<Context>, staging: Option<&Staging>) {
    let (Some(board), Some(cats)) = (held(state), state.borrow().cats.clone()) else {
        return;
    };
    let sandbox = ui.global::<Sandbox>();

    sync_deck(ui, state, context);

    let (run, runs, deleting, wanted) = {
        let held = state.borrow();
        let saved = held.saved();

        (run(&saved, &board), saved.config.jailbreak.get(&board.map_id).cloned().unwrap_or_default(), held.deleting, held.saved_search.to_lowercase())
    };
    let lineup = board.lineup(&run);
    let focus = state.borrow().jail_viewing.and_then(|unit| lineup.find(unit)).and_then(|cell| lineup.get(cell)).map(|member| (member.id, member.form));
    let saved_rows: Vec<SavedRow> = runs
        .saved
        .iter()
        .enumerate()
        .filter(|(_, saved)| wanted.is_empty() || saved.name.to_lowercase().contains(&wanted))
        .map(|(index, saved)| SavedRow { index: index as i32, name: saved.name.as_str().into(), count: format!("{}/{LINEUP_SLOTS}", board.fielded(saved).len()).into(), selected: index == runs.selected })
        .collect();

    sandbox.set_lineup_name(run.name.as_str().into());
    lineup::sync_combos(ui, context, &cats, &lineup, staging, focus, &state.borrow().combos);
    sandbox.set_saved(ModelRc::new(VecModel::from(saved_rows)));
    sandbox.set_delete_label(if deleting { lineup::CONFIRM_LABEL.into() } else { lineup::DELETE_LABEL.into() });
    ui.global::<Cats>().set_has_orbs(false);
}

pub fn recruit(state: &Shared, id: u32) {
    if let Some(board) = held(state) {
        edit(state, &board, |board, run| board.add_over(run, id));
    }
}

pub fn hovered(state: &Shared, from: Cell, to: Option<Cell>, edge: Edge) {
    let plan = held(state).zip(to).and_then(|(board, to)| {
        let run = run(&state.borrow().saved(), &board);

        board.arrange(&board.lineup(&run), Some(from), to, edge)
    });

    state.borrow_mut().plan = plan;
}

pub fn dropped(state: &Shared, from: Cell, to: Cell, edge: Edge) {
    let Some(board) = held(state) else {
        return;
    };

    if from != to {
        edit(state, &board, |board, run| {
            if let Some(plan) = board.arrange(&board.lineup(run), Some(from), to, edge) {
                board.commit(run, &plan, None);
            }
        });
    }
}

pub fn held_down(state: &Shared, cell: Cell) {
    let Some(board) = held(state) else {
        return;
    };
    let run = run(&state.borrow().saved(), &board);
    let Some(unit) = board.lineup(&run).get(cell).map(|member| member.id) else {
        return;
    };

    edit(state, &board, |board, run| board.toggle_registered(run, unit));
}

pub fn discarded(state: &Shared, cell: Cell) {
    if let Some(board) = held(state) {
        edit(state, &board, |board, run| board.discard(run, cell));
    }
}

pub fn tapped(ui: &AppWindow, state: &Shared, context: &Rc<Context>, cell: Cell) {
    let (Some(board), Some(pages)) = (held(state), context.pages()) else {
        return;
    };
    let run = run(&state.borrow().saved(), &board);
    let Some(member) = board.lineup(&run).get(cell).cloned() else {
        return;
    };
    let Some(cat) = state.borrow().cat(member.id) else {
        return;
    };
    let grants = Rc::new(board.grants(&run, member.id));

    cats::set_edited(&pages.cats, None);
    cats::seed_member(ui, &pages.cats, context, cats::Seed { cat: &cat, form: member.form, level: &member.level, talents: &member.talents, locked: true, grants: Some(Rc::clone(&grants)) });
    state.borrow_mut().jail_shown = Some((member.clone(), grants));
    state.borrow_mut().jail_viewing = Some(member.id);
    ui.global::<Sandbox>().set_unit_locked(true);
    ui.global::<Sandbox>().set_unit_open(true);
}

pub fn wire(window: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let sandbox = window.global::<Sandbox>();
    let on = |state: &Shared, context: &Rc<Context>, window: &AppWindow| (Rc::clone(state), Rc::clone(context), window.as_weak());

    let (held_state, ctx, weak) = on(state, context, window);
    sandbox.on_jail_open_bonuses(move |global| {
        held_state.borrow_mut().jail_global = global;

        if let Some(ui) = weak.upgrade() {
            ui.global::<Sandbox>().set_jail_bonuses_open(true);
            refresh(&ui, &held_state, &ctx);
        }
    });

    let (held_state, ctx, weak) = on(state, context, window);
    sandbox.on_jail_search_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held_state, &ctx);
        }
    });

    let (held_state, ctx, weak) = on(state, context, window);
    sandbox.on_jail_raise(move |id, level| {
        if let Some(board) = held(&held_state) {
            edit(&held_state, &board, |board, run| board.raise(run, id, level));
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held_state, &ctx);
        }
    });
}
