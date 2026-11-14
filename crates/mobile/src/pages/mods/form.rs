use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use tracing::{error, warn};

use foundry::common::preview::Stamp;
use foundry::systems::editor::figures::resolved::Face;
use foundry::systems::editor::{figures, prose};
use foundry::Vfs;

use crate::pages::Context;
use crate::{AppWindow, Form, FormRow, bridge};

const NUMBER: i32 = 0;
const DANGER: i32 = 1;
const TOGGLE: i32 = 2;
const CHOICE: i32 = 3;
const DISABLED: i32 = 4;
const TEXT: i32 = 5;
const HINT_TAIL: &str = "...";
const CONFIRM_MS: u64 = 3000;
const SYNC_LABEL: &str = "Sync With \"game\"";
const CONFIRM_LABEL: &str = "Continue?";

type Written = (u64, PathBuf, Option<Stamp>);
type Write = fn(&Path, &[u8], Stamp) -> Option<Stamp>;
type Job = (PathBuf, Vec<u8>, Stamp, u64);

pub enum Plan {
    Figures(figures::Plan),
    Prose(prose::Plan),
}

enum Draft {
    Figures(figures::Draft),
    Prose(prose::Draft),
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        match plan {
            Plan::Figures(plan) => figures::Draft::load(plan, vfs).filter(|draft| !draft.is_empty()).map(Draft::Figures),
            Plan::Prose(plan) => prose::Draft::load(plan, vfs).map(Draft::Prose),
        }
    }

    fn title(&self) -> &str {
        match self {
            Draft::Figures(draft) => &draft.plan.label,
            Draft::Prose(draft) => &draft.plan.label,
        }
    }

    fn notice(&self) -> &'static str {
        match self {
            Draft::Figures(_) => "",
            Draft::Prose(draft) => draft.plan.subject.notice().unwrap_or_default(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Draft::Figures(draft) => draft.len(),
            Draft::Prose(draft) => draft.plan.subject.labels().len(),
        }
    }

    fn row(&self, index: usize, held: Option<&FormRow>) -> FormRow {
        match self {
            Draft::Figures(draft) => row(draft, index, held),
            Draft::Prose(draft) => FormRow {
                label: draft.plan.subject.labels().get(index).map_or("", |label| label.trim_end_matches(HINT_TAIL)).into(),
                kind: TEXT,
                text: draft.fields.get(index).map_or("", String::as_str).into(),
                on: false,
                options: ModelRc::default(),
                current: -1,
            },
        }
    }

    fn settle(&mut self) {
        if let Draft::Figures(draft) = self {
            draft.resolve_buffer();
        }
    }

    fn changed(&mut self, index: usize, value: &str) {
        match self {
            Draft::Figures(draft) => {
                if !draft.buffering(index) {
                    draft.resolve_buffer();
                }

                draft.edit(index, value);
            }
            Draft::Prose(draft) => draft.edit(index, value.to_owned()),
        }
    }

    fn toggled(&mut self, index: usize) {
        let Draft::Figures(draft) = self else {
            return;
        };

        draft.resolve_buffer();

        if let Face::Toggle(current) = draft.face(index) {
            draft.pick(index, current.flip().raw());
        }
    }

    fn picked(&mut self, index: usize, at: usize) {
        let Draft::Figures(draft) = self else {
            return;
        };

        draft.resolve_buffer();

        if let Face::Choice(options, _) = draft.face(index)
            && let Some(choice) = options.get(at)
        {
            draft.pick(index, choice.raw);
        }
    }

    fn sync(&mut self) {
        match self {
            Draft::Figures(draft) => {
                draft.resolve_buffer();
                draft.sync();
            }
            Draft::Prose(draft) => draft.restore(),
        }
    }

    fn pending(&self) -> bool {
        match self {
            Draft::Figures(draft) => draft.dirty && !draft.writing,
            Draft::Prose(draft) => draft.dirty && !draft.writing,
        }
    }

    fn dirty(&self) -> bool {
        match self {
            Draft::Figures(draft) => draft.dirty,
            Draft::Prose(draft) => draft.dirty,
        }
    }

    fn writing(&self) -> bool {
        match self {
            Draft::Figures(draft) => draft.writing,
            Draft::Prose(draft) => draft.writing,
        }
    }

    fn failed(&self) -> bool {
        match self {
            Draft::Figures(draft) => draft.failed,
            Draft::Prose(draft) => draft.failed,
        }
    }

    fn token(&self) -> u64 {
        match self {
            Draft::Figures(draft) => draft.token,
            Draft::Prose(draft) => draft.token,
        }
    }

    fn read_from(&self) -> &Path {
        match self {
            Draft::Figures(draft) => &draft.read_from,
            Draft::Prose(draft) => &draft.read_from,
        }
    }

    fn target(&self) -> Option<&str> {
        match self {
            Draft::Figures(draft) => draft.plan.target_mod.as_deref(),
            Draft::Prose(draft) => draft.plan.target_mod.as_deref(),
        }
    }

    fn prepare(&mut self, vfs: &Vfs) -> Option<(Job, Write)> {
        match self {
            Draft::Figures(draft) => draft.prepare_write(vfs).map(|job| (job, figures::write_now as Write)),
            Draft::Prose(draft) => draft.prepare_write(vfs).map(|job| (job, prose::write_now as Write)),
        }
    }

    fn persist_now(&mut self, vfs: &Vfs) {
        match self {
            Draft::Figures(draft) => draft.persist_now(vfs),
            Draft::Prose(draft) => draft.persist_now(vfs),
        }
    }

    fn stopped(&mut self) {
        match self {
            Draft::Figures(draft) => {
                draft.writing = false;
                draft.failed = true;
            }
            Draft::Prose(draft) => {
                draft.writing = false;
                draft.failed = true;
            }
        }
    }

    fn stamped(&mut self, path: PathBuf, stamp: Option<Stamp>) {
        match self {
            Draft::Figures(draft) => {
                draft.writing = false;

                match stamp {
                    Some(stamp) => {
                        draft.read_from = path;
                        draft.stamp = stamp;
                        draft.failed = false;
                    }
                    None => draft.failed = true,
                }
            }
            Draft::Prose(draft) => {
                draft.writing = false;

                match stamp {
                    Some(stamp) => {
                        draft.read_from = path;
                        draft.stamp = stamp;
                        draft.failed = false;
                    }
                    None => draft.failed = true,
                }
            }
        }
    }
}

pub struct State {
    draft: Option<Draft>,
    leaving: Vec<Draft>,
    rows: Rc<VecModel<FormRow>>,
    written: Option<bridge::Ticket>,
    touched: bool,
    confirming: bool,
    confirm: Timer,
}

pub type Shared = Rc<RefCell<State>>;

fn row(draft: &figures::Draft, index: usize, held: Option<&FormRow>) -> FormRow {
    let mut made = FormRow {
        label: draft.label(index).as_ref().into(),
        kind: NUMBER,
        text: draft.input(index).into(),
        on: false,
        options: ModelRc::default(),
        current: -1,
    };

    match draft.face(index) {
        Face::Number => {}
        Face::Danger => made.kind = DANGER,
        Face::Toggle(current) => {
            made.kind = TOGGLE;
            made.on = current.raw() != 0;
        }
        Face::Choice(options, current) => {
            made.kind = CHOICE;
            made.current = options.iter().position(|choice| *choice == current).map_or(-1, |at| at as i32);
            made.options = match held.filter(|held| held.kind == CHOICE && held.options.row_count() == options.len()) {
                Some(held) => held.options.clone(),
                None => ModelRc::new(VecModel::from(options.iter().map(|choice| SharedString::from(choice.label())).collect::<Vec<_>>())),
            };
        }
        Face::Disabled(reason) => {
            made.kind = DISABLED;
            made.text = reason.into();
        }
    }

    made
}

fn refresh(ui: &AppWindow, state: &Shared) {
    let held = state.borrow();
    let form = ui.global::<Form>();

    let Some(draft) = held.draft.as_ref() else {
        held.rows.set_vec(Vec::new());

        return;
    };

    if held.rows.row_count() != draft.len() {
        held.rows.set_vec((0..draft.len()).map(|index| draft.row(index, None)).collect::<Vec<_>>());
    } else {
        for index in 0..draft.len() {
            let before = held.rows.row_data(index);
            let after = draft.row(index, before.as_ref());

            if before.as_ref() != Some(&after) {
                held.rows.set_row_data(index, after);
            }
        }
    }

    form.set_failed(draft.failed());
    form.set_sync_label(if held.confirming { CONFIRM_LABEL.into() } else { SYNC_LABEL.into() });
}

fn persist(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let job = {
        let mut held = state.borrow_mut();
        let ticket = held.written;
        let vault = context.engine.vault();

        held.draft.as_mut().filter(|draft| draft.pending()).and_then(|draft| draft.prepare(&vault.vfs)).zip(ticket)
    };

    let Some((((path, body, stamp, token), write), ticket)) = job else {
        return;
    };

    state.borrow_mut().touched = true;

    let weak = ui.as_weak();
    let spawned = thread::Builder::new().name("form_write".to_owned()).spawn(move || {
        let stamp = write(&path, &body, stamp);

        bridge::post::<Written>(&weak, ticket, (token, path, stamp));
    });

    if let Err(err) = spawned {
        error!("The form's write thread could not be started: {err}");

        if let Some(draft) = state.borrow_mut().draft.as_mut() {
            draft.stopped();
        }
    }
}

fn stamped(context: &Rc<Context>, draft: &mut Draft, path: PathBuf, stamp: Option<Stamp>) {
    if stamp.is_some() {
        index(context, draft, &path);
    }

    draft.stamped(path, stamp);
}

fn landed(ui: &AppWindow, state: &Shared, context: &Rc<Context>, (token, path, stamp): Written) {
    let left = {
        let mut held = state.borrow_mut();

        match held.leaving.iter().position(|draft| draft.token() == token) {
            Some(at) => Some(held.leaving.swap_remove(at)),
            None => {
                if let Some(draft) = held.draft.as_mut().filter(|draft| draft.token() == token) {
                    stamped(context, draft, path.clone(), stamp);
                }

                None
            }
        }
    };

    if let Some(mut draft) = left {
        stamped(context, &mut draft, path, stamp);
        finish(ui, context, draft);

        return;
    }

    refresh(ui, state);
    persist(ui, state, context);
}

fn index(context: &Rc<Context>, draft: &Draft, path: &Path) {
    let Some(target) = draft.target().filter(|_| draft.read_from() != path) else {
        return;
    };

    if let Err(err) = context.engine.vault().vfs.create((target, path)) {
        warn!(path = %path.display(), "Failed to index the mod's new file: {err}");
    }
}

fn finish(ui: &AppWindow, context: &Rc<Context>, mut draft: Draft) {
    let vault = context.engine.vault();
    let before = draft.read_from().to_path_buf();

    draft.settle();
    draft.persist_now(&vault.vfs);

    if draft.read_from() != before
        && let Some(target) = draft.target()
        && let Err(err) = vault.vfs.create((target, draft.read_from()))
    {
        warn!(path = %draft.read_from().display(), "Failed to index the mod's new file: {err}");
    }

    if let Some(name) = draft.read_from().file_name() {
        context.changed(ui, &name.to_string_lossy());
    }
}

fn settle(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    let closed = {
        let mut held = state.borrow_mut();
        let touched = std::mem::take(&mut held.touched);

        held.confirming = false;
        held.confirm.stop();

        match held.draft.take() {
            Some(draft) if draft.writing() => {
                held.leaving.push(draft);

                None
            }
            Some(mut draft) => {
                draft.settle();

                (touched || draft.dirty()).then_some(draft)
            }
            None => None,
        }
    };

    refresh(ui, state);

    if let Some(draft) = closed {
        finish(ui, context, draft);
    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) -> Shared {
    let rows = Rc::new(VecModel::default());
    let state: Shared = Rc::new(RefCell::new(State { draft: None, leaving: Vec::new(), rows: Rc::clone(&rows), written: None, touched: false, confirming: false, confirm: Timer::default() }));
    let form = window.global::<Form>();

    form.set_rows(ModelRc::from(rows));

    let (held, ctx) = (Rc::clone(&state), Rc::clone(context));
    let written = bridge::subscribe::<Written>(move |ui, outcome| landed(ui, &held, &ctx, outcome));

    state.borrow_mut().written = Some(written);

    let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());
    form.on_changed(move |index, value| {
        if let Some(draft) = held.borrow_mut().draft.as_mut() {
            draft.changed(index.max(0) as usize, &value);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
            persist(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());
    form.on_toggled(move |index| {
        if let Some(draft) = held.borrow_mut().draft.as_mut() {
            draft.toggled(index.max(0) as usize);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
            persist(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());
    form.on_picked(move |index, at| {
        if let Some(draft) = held.borrow_mut().draft.as_mut() {
            draft.picked(index.max(0) as usize, at.max(0) as usize);
        }

        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &held);
            persist(&ui, &held, &ctx);
        }
    });

    let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());
    form.on_sync(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };

        let armed = std::mem::replace(&mut held.borrow_mut().confirming, false);

        if !armed {
            let (landing, weak) = (Rc::clone(&held), ui.as_weak());

            {
                let mut state = held.borrow_mut();

                state.confirming = true;
                state.confirm.start(TimerMode::SingleShot, Duration::from_millis(CONFIRM_MS), move || {
                    landing.borrow_mut().confirming = false;

                    if let Some(ui) = weak.upgrade() {
                        refresh(&ui, &landing);
                    }
                });
            }

            refresh(&ui, &held);

            return;
        }

        if let Some(draft) = held.borrow_mut().draft.as_mut() {
            draft.sync();
        }

        refresh(&ui, &held);
        persist(&ui, &held, &ctx);
    });

    let (held, ctx, weak) = (Rc::clone(&state), Rc::clone(context), window.as_weak());
    form.on_closed(move || {
        if let Some(ui) = weak.upgrade() {
            settle(&ui, &held, &ctx);
        }
    });

    state
}

pub fn open(ui: &AppWindow, state: &Shared, context: &Rc<Context>, plan: Plan) -> bool {
    settle(ui, state, context);

    let Some(draft) = Draft::load(plan, &context.engine.vault().vfs) else {
        return false;
    };

    let form = ui.global::<Form>();

    form.set_title(draft.title().into());
    form.set_notice(draft.notice().into());
    state.borrow_mut().draft = Some(draft);
    refresh(ui, state);
    form.set_open(true);

    true
}

pub fn shut(ui: &AppWindow, state: &Shared, context: &Rc<Context>) {
    if state.borrow().draft.is_none() {
        return;
    }

    settle(ui, state, context);
    ui.global::<Form>().set_open(false);
}
