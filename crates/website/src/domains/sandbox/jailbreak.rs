use std::rc::Rc;

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

use emulator::runtime::{RoguelikePicks, RosterPick, SetupUnit};
use foundry::domains::cat::scanner::{self, CatEntry};
use foundry::domains::sandbox::jailbreak::{self as rules, Board, Run, Runs};
use foundry::domains::sandbox::{Arrangement, BENCH_SLOTS, Cell, LINEUP_SLOTS, Member, Placed};
use foundry::systems::combat::registry::AbilityIcon;

use crate::domains::cat::{self, FilterPopup, SelectedCat, icons};
use crate::platform::Platform;
use crate::systems::context::Shared;
use crate::systems::images;
use crate::systems::picture::Pic;
use crate::systems::turn;
use crate::widget::{Drawer, Popup, Slider, Tip, drawer};

use super::combos;

use super::lineup::{landing, Cargo, Drag, Landing, Metrics, SandboxRoster, BADGE_SIZE, CARD_BORDER, COLUMNS, CONFIRM_MS, DOUBLE_CLICK_MS, LEVEL_SIZE, RIPE_MS, SLACK, TOUCH, UNIT_WIDTH};
use super::state::{self, SandboxState};
use super::{Cats, Dialog, Ruled};

const BONUS_WIDTH: f32 = 700.0;
const ICON_SIZE: f32 = 36.0;
const EVERY_FORM: usize = 3;
const CONFIRM_LABEL: &str = "Continue?";
const HOLD_MS: u32 = 700;
const NO_BONUSES: &str = "No bonus matches";

#[derive(Clone)]
pub(crate) struct Held(pub(crate) Rc<Board>);

impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Jail(pub(crate) Signal<Option<Held>>);

pub(crate) fn picks(state: &SandboxState, board: &Board) -> (RoguelikePicks, Vec<SetupUnit>) {
    let run = state.config.jailbreak.get(&board.map_id).and_then(Runs::current).map_or_else(Run::default, |run| board.settled(run));
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

fn settled(sandbox: Signal<SandboxState>, board: &Board) -> Run {
    sandbox.read().config.jailbreak.get(&board.map_id).and_then(Runs::current).map_or_else(Run::default, |run| board.settled(run))
}

fn edit(platform: &Platform, sandbox: Signal<SandboxState>, board: &Board, change: impl FnOnce(&Board, &mut Run)) {
    state::apply(platform, sandbox, |next| {
        let run = next.config.jailbreak.entry(board.map_id).or_default().current_mut();

        board.settle(run);
        change(board, run);
    });
}

fn shelve(platform: &Platform, sandbox: Signal<SandboxState>, board: &Board, change: impl FnOnce(&mut Runs)) {
    state::apply(platform, sandbox, |next| change(next.config.jailbreak.entry(board.map_id).or_default()));
}

#[component]
pub(crate) fn Editor(board: Held, cats: Cats, globals: Shared, ruled: Ruled) -> Element {
    let platform = use_context::<Platform>();
    let sandbox = state::state();
    let pictures = images::pictures();
    let mut cat_state = cat::provide_locked();
    let mut drag = use_signal(Drag::default);
    let mut plan_held: Signal<Option<Arrangement>> = use_signal(|| None);
    let mut viewing: Signal<Option<u32>> = use_signal(|| None);
    let mut clicked: Signal<Option<(Cell, f64)>> = use_signal(|| None);
    let mut deck_origin: Signal<Option<(f64, f64)>> = use_signal(|| None);
    let mut deck_mount: Signal<Option<MountedEvent>> = use_signal(|| None);
    let mut area = use_signal(|| (0f32, 0f32));
    let mut roster_open = use_signal(|| false);
    let mut saved_open = use_signal(|| false);
    let mut global_open = use_signal(|| false);
    let mut cat_open = use_signal(|| false);
    let mut search = use_signal(String::new);
    let mut deleting = use_signal(|| false);
    let mut delete_round = use_signal(|| 0u32);
    let drawers = drawer::drawers();
    let turn = turn::turn();
    let run = settled(sandbox, &board.0);
    let runs = sandbox.read().config.jailbreak.get(&board.0.map_id).cloned().unwrap_or_default();
    let selected_run = runs.selected;
    let wanted = search().to_lowercase();
    let found: Vec<(usize, String, usize)> = runs
        .saved
        .iter()
        .enumerate()
        .filter(|(_, saved)| wanted.is_empty() || saved.name.to_lowercase().contains(&wanted))
        .map(|(index, saved)| (index, saved.name.clone(), board.0.fielded(saved).len()))
        .collect();
    let named = run.name.clone();
    let lineup = Rc::new(board.0.lineup(&run));
    let mut forms: Vec<(u32, usize)> = board.0.cats.iter().map(|cat| (cat.unit, board.0.standing(&run, cat.unit).form)).collect();

    forms.sort_unstable();

    let listing = cats.clone();
    let listed = use_memo(use_reactive!(|(listing, forms)| {
        let shown: Vec<CatEntry> = listing
            .0
            .iter()
            .filter_map(|cat| {
                let form = forms.binary_search_by_key(&cat.id, |(unit, _)| *unit).ok().and_then(|found| forms.get(found)).map(|(_, form)| *form)?;
                let mut shown = cat.clone();

                for later in form.saturating_add(1)..shown.banner_paths.len() {
                    shown.banner_paths[later] = None;
                    shown.forms[later] = false;
                }

                Some(shown)
            })
            .collect();

        Cats(Rc::new(shown))
    }));
    let listed = listed();
    let (area_width, area_height) = area();
    let beside = (drawers.compact)() && area_width > area_height;
    let metrics = if beside { Metrics::beside(area_width, area_height, true) } else { Metrics::fitting(area_width, area_height, true) };
    let dragging = drag();
    let lifted = match dragging {
        Drag::Moving { lifted, .. } => lifted,
        _ => None,
    };
    let arrangement = plan_held();
    let icon_of = {
        let pictures = pictures.clone();
        let cats = Rc::clone(&cats.0);
        let globals = globals.clone();

        move |id: u32, form: usize| -> Option<Rc<str>> {
            let vfs = &globals.0.vault().vfs;
            let path = match cats.iter().find(|cat| cat.id == id) {
                Some(cat) => scanner::deploy_icon(vfs, cat, form),
                None => scanner::orphan_icon(vfs, id, form),
            };

            path.and_then(|path| pictures.trimmed(&path)).map(|sized| sized.url)
        }
    };
    let refresh_deck = move || {
        if let Some(mounted) = deck_mount.peek().clone() {
            spawn(async move {
                if let Ok(rect) = mounted.get_client_rect().await {
                    let area = turn.area(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);

                    deck_origin.set(Some((area.x, area.y)));
                }
            });
        }
    };
    let press = {
        let platform = platform.clone();

        move |cargo: Cargo, at: (f64, f64), touch: bool| {
            let now = platform.host().now();

            refresh_deck();
            drag.set(Drag::Pressed { cargo, since: now, from: at, touch });
        }
    };
    let planned = {
        let board = board.clone();
        let lineup = Rc::clone(&lineup);

        move |lifted: Option<Cell>, landing: Landing| -> Option<Arrangement> {
            let Landing::Cell(target, edge) = landing else {
                return None;
            };

            board.0.arrange(&lineup, lifted, target, edge)
        }
    };
    let moved = {
        let platform = platform.clone();
        let lineup = Rc::clone(&lineup);

        Callback::new(move |at: (f64, f64)| {
            if *drag.peek() == Drag::Idle {
                return;
            }

            let now = platform.host().now();
            let origin = *deck_origin.peek();
            let next = match *drag.peek() {
                Drag::Pressed { cargo, since, from, touch } => {
                    let (across, down) = ((at.0 - from.0).abs(), (at.1 - from.1).abs());
                    let strayed = across > SLACK || down > SLACK;
                    let scrolling = touch && matches!(cargo, Cargo::Fresh(_));

                    if scrolling && down > SLACK && down >= across {
                        plan_held.set(None);

                        Drag::Idle
                    } else if (scrolling && across > SLACK) || (!scrolling && (strayed || (matches!(cargo, Cargo::Fresh(_)) && now - since >= RIPE_MS))) {
                        let landing = landing(at, origin, metrics);
                        let lifted = match cargo {
                            Cargo::Held(cell) => Some(cell),
                            Cargo::Fresh(id) => lineup.find(id),
                        };

                        plan_held.set(planned(lifted, landing));

                        if matches!(cargo, Cargo::Fresh(_)) {
                            roster_open.set(false);
                        }

                        Drag::Moving { cargo, lifted, at, landing }
                    } else {
                        Drag::Pressed { cargo, since, from, touch }
                    }
                }
                Drag::Moving { cargo, lifted, landing: before, .. } => {
                    let landing = landing(at, origin, metrics);

                    if landing != before {
                        plan_held.set(planned(lifted, landing));
                    }

                    Drag::Moving { cargo, lifted, at, landing }
                }
                Drag::Idle => Drag::Idle,
            };

            drag.set(next);
        })
    };
    let released = {
        let platform = platform.clone();
        let board = board.clone();
        let lineup = Rc::clone(&lineup);

        Callback::new(move |()| {
            if *drag.peek() == Drag::Idle {
                return;
            }

            let settled = std::mem::take(&mut *drag.write());
            let plan = plan_held.take();

            match settled {
                Drag::Pressed { cargo: Cargo::Fresh(id), .. } if *drawers.compact.peek() => edit(&platform, sandbox, &board.0, |board, run| board.add_over(run, id)),
                Drag::Pressed { cargo: Cargo::Fresh(id), .. } => edit(&platform, sandbox, &board.0, |board, run| board.add(run, id)),
                Drag::Pressed { cargo: Cargo::Held(cell), .. } => {
                    let now = platform.host().now();
                    let doubled = clicked.peek().is_some_and(|(held, at)| held == cell && now - at <= DOUBLE_CLICK_MS);

                    clicked.set((!doubled).then_some((cell, now)));

                    if doubled {
                        viewing.set(lineup.get(cell).map(|member| member.id));
                    }
                }
                Drag::Moving { cargo, lifted, landing: Landing::Cell(..), .. } => {
                    let Some(plan) = plan else {
                        return;
                    };
                    let fresh = match cargo {
                        Cargo::Fresh(id) if lifted.is_none() => Some(id),
                        _ => None,
                    };

                    edit(&platform, sandbox, &board.0, |board, run| board.commit(run, &plan, fresh));
                }
                Drag::Moving { cargo: Cargo::Held(from), landing: Landing::Off, .. } => edit(&platform, sandbox, &board.0, |board, run| board.discard(run, from)),
                _ => {}
            }
        })
    };
    let cancelled = Callback::new(move |()| {
        drag.set(Drag::Idle);
        plan_held.set(None);
    });
    let shown = |cell: Cell| -> (Option<Member>, bool) {
        let source = arrangement.as_ref().map_or_else(|| (lifted != Some(cell)).then_some(Placed::Kept(cell)), |plan| plan.get(cell));

        match source {
            Some(Placed::Kept(source)) => (lineup.get(source).cloned(), false),
            Some(Placed::Carried) => (None, true),
            None => (None, false),
        }
    };
    let enlisted = run.registered.clone();
    let cell_view = |cell: Cell| -> Element {
        let (member, aimed) = shown(cell);
        let frame_class = if aimed { "deck-cell aimed" } else { "deck-cell" };
        let size_style = format!("width: {}px; height: {}px;", metrics.cell_width, metrics.cell_height);
        let mut press = press.clone();

        let Some(member) = member else {
            return rsx! {
                div { key: "{cell:?}", class: "{frame_class}", style: "{size_style}" }
            };
        };
        let icon = icon_of(member.id, member.form);
        let unit = member.id;
        let registered = enlisted.contains(&unit);
        let hold_platform = platform.clone();
        let hold_board = board.clone();
        let (level, plus) = member.levels(None);
        let badge = if plus > 0 { format!("Lv{level}+{plus}") } else { format!("Lv{level}") };
        let fallback = format!("{:03}-{}", member.id, member.form + 1);

        rsx! {
            div {
                key: "{cell:?}",
                class: "{frame_class} filled",
                style: "{size_style}",
                onpointerdown: move |event| {
                    if event.data().trigger_button() != Some(MouseButton::Primary) {
                        return;
                    }

                    event.prevent_default();
                    drawers.claim(event.data().pointer_id());
                    let point = event.data().client_coordinates();

                    press(Cargo::Held(cell), turn.point(point.x, point.y), event.data().pointer_type() == TOUCH);

                    let began = match *drag.peek() {
                        Drag::Pressed { since, .. } => since,
                        _ => return,
                    };
                    let sleeper = hold_platform.clone();
                    let board = hold_board.clone();

                    spawn(async move {
                        sleeper.host().sleep(HOLD_MS).await;

                        if *drag.peek() != (Drag::Pressed { cargo: Cargo::Held(cell), since: began, from: turn.point(point.x, point.y), touch: event.data().pointer_type() == TOUCH }) {
                            return;
                        }

                        drag.set(Drag::Idle);
                        plan_held.set(None);
                        clicked.set(None);
                        edit(&sleeper, sandbox, &board.0, |board, run| board.toggle_registered(run, unit));
                    });
                },
                onpointermove: move |event| {
                    if *drag.peek() != Drag::Idle {
                        let point = event.data().client_coordinates();

                        moved.call(turn.point(point.x, point.y));
                    }
                },
                onpointerup: move |_| {
                    if *drag.peek() != Drag::Idle {
                        released.call(());
                    }
                },
                onpointercancel: move |_| {
                    if *drag.peek() != Drag::Idle {
                        cancelled.call(());
                    }
                },
                match icon {
                    Some(url) => rsx! { Pic { url, class: "deck-face" } },
                    None => rsx! { div { class: "deck-fallback", style: "font-size: {LEVEL_SIZE}px;", "{fallback}" } },
                }
                div { class: "deck-badge deck-level", style: "font-size: {metrics.text(BADGE_SIZE)}px;", "{badge}" }
                if registered {
                    div { class: "deck-badge deck-registered", style: "font-size: {metrics.text(BADGE_SIZE)}px;", "{rules::REGISTERED}" }
                }
            }
        }
    };
    let ghost = match dragging {
        Drag::Moving { cargo, at, .. } => {
            let url = match cargo {
                Cargo::Held(cell) => lineup.get(cell).and_then(|member| icon_of(member.id, member.form)),
                Cargo::Fresh(id) => icon_of(id, board.0.standing(&run, id).form),
            };

            Some((url, at))
        }
        _ => None,
    };
    let mut press_row = press.clone();
    let filter_class = if cat_state.filter.read().is_active() { "btn btn-toggle active roster-filter" } else { "btn btn-toggle roster-filter" };
    let viewed = viewing().and_then(|id| lineup.find(id)).and_then(|cell| lineup.get(cell).cloned());
    let grants_board = board.clone();
    let grants_cats = cats.clone();
    let seen = use_memo(use_reactive!(|(viewed, run)| {
        let member = viewed?;
        let grants = grants_board.0.grants(&run, member.id);
        let mut cat = grants_cats.0.iter().find(|cat| cat.id == member.id)?.clone();

        cat.talent_data = (!grants.talent.groups.is_empty()).then(|| grants.talent.clone());

        Some((member, SelectedCat(Rc::new(cat)), Rc::new(grants)))
    }));
    let seen = seen();
    let seeded = seen.clone();

    use_effect(use_reactive!(|seeded| {
        if let Some((member, cat, grants)) = seeded {
            cat_state.seed(&cat.0, member.form, &member.level, &member.talents);
            cat_state.granted.set(Some(grants));
        }
    }));

    let focus = seen.as_ref().map(|(member, ..)| (member.id, member.form));
    let delete_label = if deleting() { CONFIRM_LABEL } else { "Delete" };
    let create_platform = platform.clone();
    let create_board = board.clone();
    let delete_platform = platform.clone();
    let delete_board = board.clone();
    let rename_platform = platform.clone();
    let rename_board = board.clone();

    rsx! {
        div { class: "lineup-editor",
            if dragging != Drag::Idle {
                div {
                    class: "drag-scrim",
                    onpointermove: move |event| {
                        let point = event.data().client_coordinates();

                        moved.call(turn.point(point.x, point.y));
                    },
                    onpointerup: move |_| released.call(()),
                    onpointercancel: move |_| cancelled.call(()),
                    onpointerleave: move |_| cancelled.call(()),
                }
            }
            if let Some((url, (x, y))) = ghost {
                div { class: "lineup-ghost", style: "left: {x as f32 - metrics.cell_width / 2.0}px; top: {y as f32 - metrics.cell_height / 2.0}px; width: {metrics.cell_width}px; height: {metrics.cell_height}px;",
                    if let Some(url) = url {
                        Pic { url, class: "deck-face" }
                    }
                }
            }
            Drawer { open: roster_open,
                SandboxRoster {
                    cats: listed.clone(),
                    banner_form: EVERY_FORM,
                    filter_class: filter_class.to_owned(),
                    onpress: Callback::new(move |(id, at, touch): (u32, (f64, f64), bool)| press_row(Cargo::Fresh(id), at, touch)),
                    onmove: moved,
                    onrelease: released,
                    oncancel: cancelled,
                    onsearch: Callback::new(move |_: String| {}),
                }
            }
            div {
                class: if beside { "lineup-center beside" } else { "lineup-center" },
                onresize: move |event| {
                    if let Ok(size) = event.data().get_content_box_size() {
                        area.set((size.width as f32, size.height as f32));
                    }
                },
                div { class: "lineup-deck", style: "width: {metrics.deck_width()}px;",
                    div { class: "lineup-bar",
                        span { class: "lineup-name", "{named}" }
                        button { class: "btn btn-neutral lineup-saved-btn", onclick: move |_| saved_open.toggle(), "Lineups" }
                    }
                    div {
                        class: "outlined-card deck-card",
                        style: "width: {metrics.deck_width()}px; height: {metrics.deck_height()}px; padding: {metrics.pad - CARD_BORDER}px;",
                        onmounted: move |event| deck_mount.set(Some(event)),
                        div { class: "deck-title bold", style: "height: {metrics.header}px; font-size: {metrics.text(16.0)}px;", "Lineup" }
                        div { style: "height: {metrics.header_gap}px;" }
                        div { class: "deck-actions", style: "height: {metrics.actions}px;",
                            button { class: "btn btn-primary deck-action", title: rules::GLOBAL_HINT, onclick: move |_| global_open.set(true), "{rules::GLOBAL_LABEL}" }
                            button { class: "btn btn-primary deck-action", title: rules::CAT_HINT, onclick: move |_| cat_open.set(true), "{rules::CAT_LABEL}" }
                        }
                        div { class: "deck-grid", style: "gap: {metrics.gap}px;",
                            for line in 0..LINEUP_SLOTS / COLUMNS {
                                div { key: "{line}", class: "deck-row", style: "gap: {metrics.gap}px;",
                                    for column in 0..COLUMNS {
                                        {cell_view(Cell::Slot(line * COLUMNS + column))}
                                    }
                                }
                            }
                        }
                        div { style: "height: {metrics.bench_gap}px;" }
                        div { class: "deck-title bold", style: "height: {metrics.bench_title}px; font-size: {metrics.text(14.0)}px;", "Bench" }
                        div { class: "deck-note weak", style: "height: {metrics.bench_note}px; font-size: {metrics.text(LEVEL_SIZE)}px;", "{rules::BENCH_NOTE}" }
                        div { style: "height: {metrics.gap}px;" }
                        div { class: "deck-row", style: "gap: {metrics.gap}px;",
                            for index in 0..BENCH_SLOTS {
                                {cell_view(Cell::Bench(index))}
                            }
                        }
                    }
                }
                combos::Combos { cats: cats.clone(), globals: globals.clone(), rules: ruled.clone(), lineup: combos::Held(Rc::clone(&lineup)), focus, onadopt: move |_: usize| {} }
            }
            if saved_open() {
                div { class: "drawer-scrim", onclick: move |_| saved_open.set(false) }
            }
            div { class: if saved_open() { "list-panel-right saved-panel shown" } else { "list-panel-right saved-panel" },
                div { class: "saved-actions",
                    button { class: "btn btn-success saved-btn", onclick: move |_| {
                        shelve(&create_platform, sandbox, &create_board.0, Runs::create);
                        viewing.set(None);
                    }, "New" }
                    button { class: "btn btn-danger saved-btn", onclick: move |_| {
                        if !deleting() {
                            let round = delete_round() + 1;

                            delete_round.set(round);
                            deleting.set(true);

                            let sleeper = delete_platform.clone();

                            spawn(async move {
                                sleeper.host().sleep(CONFIRM_MS).await;

                                if delete_round() == round {
                                    deleting.set(false);
                                }
                            });

                            return;
                        }

                        deleting.set(false);
                        shelve(&delete_platform, sandbox, &delete_board.0, Runs::delete);
                        viewing.set(None);
                    }, "{delete_label}" }
                }
                input { class: "input saved-search", r#type: "search", placeholder: "Search Lineup...", value: "{search}", oninput: move |event| search.set(event.value()) }
                div { class: "saved-list scroll",
                    for (index, name, count) in found {
                        {
                            let load_platform = platform.clone();
                            let load_board = board.clone();
                            let class = if index == selected_run { "list-row filled selected saved-row" } else { "list-row filled saved-row" };

                            rsx! {
                                button { key: "{index}", class: "{class}", onclick: move |_| {
                                    shelve(&load_platform, sandbox, &load_board.0, |runs| runs.load(index));
                                    deleting.set(false);
                                    viewing.set(None);
                                    saved_open.set(false);
                                },
                                    span { class: "saved-name", "{name}" }
                                    span { class: "weak saved-count", "{count}/{LINEUP_SLOTS}" }
                                }
                            }
                        }
                    }
                }
                input { class: "input saved-rename", r#type: "text", placeholder: "Lineup Name", value: "{named}", oninput: move |event| shelve(&rename_platform, sandbox, &rename_board.0, |runs| runs.current_mut().name = event.value()) }
            }
        }
        if global_open() {
            BonusPopup { board: board.clone(), open: global_open, global: true }
        }
        if cat_open() {
            BonusPopup { board: board.clone(), open: cat_open, global: false }
        }
        FilterPopup { cats: cat::list::Entries(Rc::clone(&listed.0)), globals: globals.clone() }
        if let Some((member, cat, _)) = seen {
            Dialog { title: cat.0.display_name(member.form), width: UNIT_WIDTH, class: "unit-popup", onclose: move |()| viewing.set(None),
                cat::Detail { cat, globals: globals.clone() }
            }
        }
    }
}

type Offers = Vec<(String, Vec<(usize, i32, String)>)>;

fn grouped(board: &Board, run: &Run, query: &str, global: bool) -> Offers {
    let wanted = query.trim().to_lowercase();
    let mut groups: Offers = Vec::new();

    for (index, bonus) in board.bonuses.iter().enumerate() {
        if bonus.target.is_none() != global || !board.offers(run, bonus) {
            continue;
        }

        if !(wanted.is_empty() || board.label(run, bonus).to_lowercase().contains(&wanted)) {
            continue;
        }

        let title = board.title(run, bonus);
        let offer = (index, run.level(bonus.id), board.detail(run, bonus));

        match groups.iter_mut().find(|(held, _)| *held == title) {
            Some((_, offers)) => offers.push(offer),
            None => groups.push((title, vec![offer])),
        }
    }

    groups
}

#[component]
fn BonusPopup(board: Held, open: Signal<bool>, global: bool) -> Element {
    let platform = use_context::<Platform>();
    let sandbox = state::state();
    let pictures = images::pictures();
    let mut query = use_signal(String::new);
    let run = settled(sandbox, &board.0);
    let searched = query();
    let shown_board = board.clone();
    let sought = searched.clone();
    let groups = use_memo(use_reactive!(|(run, sought, global)| Rc::new(grouped(&shown_board.0, &run, &sought, global))));
    let title = if global { rules::GLOBAL_TITLE } else { rules::CAT_TITLE };
    let groups = groups();

    rsx! {
        Popup { title, open, width: BONUS_WIDTH, class: "jail-popup",
            div { class: "jail-bonuses",
                input { class: "input", r#type: "search", placeholder: "Search bonuses", value: "{searched}", oninput: move |event| query.set(event.value()) }
                if groups.is_empty() {
                    div { class: "weak", "{NO_BONUSES}" }
                }
                for (title, offers) in groups.iter() {
                    div { key: "{title}", class: "bonus-group",
                        div { class: "bold bonus-group-title", "{title}" }
                        div { class: "bonus-tiles",
                            for (index, level, label) in offers.iter().cloned() {
                                if let Some(bonus) = board.0.bonuses.get(index) {
                                    {
                                        let id = bonus.id;
                                        let limit = bonus.limit;
                                        let tile_platform = platform.clone();
                                        let tile_board = board.clone();

                                        rsx! {
                                            div { key: "{id}", class: "card-muted bonus-tile",
                                                Tip { text: label, hover: true,
                                                    match bonus.icon {
                                                        AbilityIcon::Standard(cut) => match icons::sheet_url(&pictures, bonus.sheet, cut) {
                                                            Some(url) => rsx! { Pic { url, class: "ability-icon bonus-icon", style: format!("width: {ICON_SIZE}px; height: {ICON_SIZE}px;") } },
                                                            None => icons::fallback(bonus.fallback, ICON_SIZE),
                                                        },
                                                        AbilityIcon::Custom(_) | AbilityIcon::None => icons::fallback(bonus.fallback, ICON_SIZE),
                                                    }
                                                }
                                                div { class: "bonus-level",
                                                    div { class: "bonus-row",
                                                        Slider { class: "bonus-slider", min: 0, max: limit, value: level, oninput: move |level: i32| edit(&tile_platform, sandbox, &tile_board.0, |board, run| board.raise(run, id, level)) }
                                                        span { class: "bonus-count", "{level}/{limit}" }
                                                    }
                                                    if bonus.permanent {
                                                        span { class: "weak bonus-tag", "{rules::PERMANENT}" }
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
