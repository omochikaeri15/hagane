mod cats;
mod detail;
mod enemies;
mod files;
mod help;
mod home;
mod import;
mod mods;
mod sandbox;
mod settings;
mod stages;
mod utilities;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tracing::info;

use foundry::common::architecture;

use crate::catalog::Catalogs;
use crate::context::Globals;
use crate::engine::Engine;
use crate::images::Images;
use crate::memory::Memory;
use crate::settings::State;
use crate::{picker, update, viewer};
use crate::{AppWindow, Page, Shell, bridge};

const TOAST_MS: u64 = 3000;

pub struct Pages {
    pub cats: cats::Shared,
    pub enemies: enemies::Shared,
    pub files: files::Shared,
    pub utilities: utilities::Shared,
    pub stages: stages::Shared,
    pub sandbox: sandbox::Shared,
    pub settings: settings::Shared,
}

pub struct Context {
    pub state: Rc<State>,
    pub memory: Rc<Memory>,
    pub engine: Rc<Engine>,
    pub images: Images,
    pub catalogs: Catalogs,
    pub viewer: viewer::Shared,
    pub appearances: Cell<Option<u32>>,
    pub picker: picker::Shared,
    pages: RefCell<Option<Rc<Pages>>>,
    globals: RefCell<Option<Rc<Globals>>>,
    timers: RefCell<Vec<Timer>>,
    toast: RefCell<Option<Timer>>,
    landing: RefCell<Option<bridge::Ticket>>,
    soft: RefCell<Option<bridge::Ticket>>,
}

impl Context {
    pub fn new(window: &AppWindow, state: Rc<State>, memory: Rc<Memory>, engine: Rc<Engine>) -> Rc<Self> {
        let context = Rc::new(Self {
            state,
            memory,
            engine,
            images: Images::new(window),
            catalogs: Catalogs::new(),
            viewer: viewer::State::shared(),
            appearances: Cell::new(None),
            picker: picker::Shared::default(),
            pages: RefCell::new(None),
            globals: RefCell::new(None),
            timers: RefCell::new(Vec::new()),
            toast: RefCell::new(None),
            landing: RefCell::new(None),
            soft: RefCell::new(None),
        });
        let weak: Weak<Self> = Rc::downgrade(&context);
        let ticket = bridge::subscribe::<(u64, Globals)>(move |ui, (generation, globals)| {
            if let Some(context) = weak.upgrade()
                && context.engine.shared().generation() == generation
            {
                context.adopt(ui, globals);
            }
        });

        let weak: Weak<Self> = Rc::downgrade(&context);
        let soft = bridge::subscribe::<(u64, Globals)>(move |ui, (generation, globals)| {
            if let Some(context) = weak.upgrade()
                && context.engine.shared().generation() == generation
            {
                *context.globals.borrow_mut() = Some(Rc::new(globals));
                context.catalogs.notify(ui);
            }
        });

        *context.landing.borrow_mut() = Some(ticket);
        *context.soft.borrow_mut() = Some(soft);
        context
    }

    pub fn globals(&self) -> Option<Rc<Globals>> {
        self.globals.borrow().clone()
    }

    pub fn pages(&self) -> Option<Rc<Pages>> {
        self.pages.borrow().clone()
    }

    pub fn keep_timer(&self, timer: Timer) {
        let mut timers = self.timers.borrow_mut();

        timers.retain(Timer::running);
        timers.push(timer);
    }

    pub fn toast(&self, ui: &AppWindow, text: &str, danger: bool) {
        self.toast_detail(ui, text, "", danger);
    }

    pub fn toast_detail(&self, ui: &AppWindow, text: &str, detail: &str, danger: bool) {
        let shell = ui.global::<Shell>();

        shell.set_toast_text(text.into());
        shell.set_toast_detail(detail.into());
        shell.set_toast_danger(danger);
        shell.set_toast_shown(true);

        let weak = ui.as_weak();
        let timer = Timer::default();

        timer.start(TimerMode::SingleShot, Duration::from_millis(TOAST_MS), move || {
            if let Some(ui) = weak.upgrade() {
                ui.global::<Shell>().set_toast_shown(false);
            }
        });
        *self.toast.borrow_mut() = Some(timer);
    }

    fn landed(&self, ui: &AppWindow) {
        let Some(ticket) = *self.landing.borrow() else {
            return;
        };
        let weak = ui.as_weak();
        let settings = self.state.snapshot();

        ui.global::<Shell>().set_game_empty(!architecture::game_present());
        self.engine.run(move |shared| {
            let vault = shared.vault();
            let globals = Globals::build(&vault, &settings);

            bridge::post(&weak, ticket, (shared.generation(), globals));
        });
    }

    fn adopt(self: &Rc<Self>, ui: &AppWindow, globals: Globals) {
        info!("Engine data adopted, catalogs restart");
        *self.globals.borrow_mut() = Some(Rc::new(globals));
        self.images.forget_all();
        viewer::forget(self);
        self.catalogs.reset(ui);
        self.entered(ui, ui.global::<Shell>().get_page());

        if let Some(pages) = self.pages() {
            utilities::invalidate(ui, &pages.utilities, self);
            sandbox::invalidate(ui, &pages.sandbox, self);
            settings::invalidate(ui, &pages.settings, self);
        }
    }

    pub fn reload_globals(&self, ui: &AppWindow) {
        self.landed(ui);
    }

    pub fn retreasure(&self, ui: &AppWindow) {
        let Some(ticket) = *self.soft.borrow() else {
            return;
        };
        let weak = ui.as_weak();
        let settings = self.state.snapshot();

        self.engine.run(move |shared| {
            let vault = shared.vault();
            let globals = Globals::build(&vault, &settings);

            bridge::post(&weak, ticket, (shared.generation(), globals));
        });
    }

    fn entered(self: &Rc<Self>, ui: &AppWindow, page: Page) {
        let config = self.state.settings().scanner_config(self.engine.mounted());

        if let Some(pages) = self.pages() {
            if page != Page::Files {
                files::leave(&pages.files, self);
            }

            if page != Page::Utilities {
                utilities::left(&pages.utilities, self);
            }

            if page != Page::Sandbox {
                sandbox::left(&pages.sandbox);
            }
        }

        if page != Page::Cats && page != Page::Enemies && page != Page::Utilities {
            viewer::hide(self);
        }

        match page {
            Page::Cats => {
                self.catalogs.load_cats(ui, &self.engine, config);

                if let Some(pages) = self.pages() {
                    cats::entered(ui, &pages.cats, self);
                }
            }
            Page::Enemies => {
                self.catalogs.load_enemies(ui, &self.engine, config);

                if let Some(pages) = self.pages() {
                    enemies::entered(ui, &pages.enemies, self);
                }
            }
            Page::Stages => self.catalogs.load_stages(ui, &self.engine, config),
            Page::Files => {
                if let Some(pages) = self.pages() {
                    files::sync(ui, &pages.files, self);
                }
            }
            Page::Utilities => {
                if let Some(pages) = self.pages() {
                    utilities::entered(ui, &pages.utilities, self);
                }
            }
            Page::Sandbox => {
                if let Some(pages) = self.pages() {
                    sandbox::entered(ui, &pages.sandbox, self);
                }
            }
            _ => {}
        }

    }
}

pub fn wire(window: &AppWindow, context: &Rc<Context>) {
    let shell = window.global::<Shell>();

    {
        let (weak, context) = (window.as_weak(), Rc::clone(context));

        shell.on_navigate(move |page| {
            if let Some(ui) = weak.upgrade() {
                ui.global::<Shell>().set_page(page);
                context.entered(&ui, page);
            }
        });
    }

    {
        let (weak, context) = (window.as_weak(), Rc::clone(context));

        shell.on_engine_landed(move || {
            if let Some(ui) = weak.upgrade() {
                context.landed(&ui);
            }
        });
    }

    home::wire(window, context);
    viewer::wire(window, context);
    let cats = cats::wire(window, context);
    let enemies = enemies::wire(window, context);
    let files = files::wire(window, context);
    let utilities = utilities::wire(window, context);

    let stages = stages::wire(window, context);
    let sandbox = sandbox::wire(window, context);

    mods::wire(window, context);
    import::wire(window, context);
    help::wire(window, context);
    let settings = settings::wire(window, context);
    picker::wire(window, context);
    update::wire(window, context);
    *context.pages.borrow_mut() = Some(Rc::new(Pages { cats, enemies, files, utilities, stages, sandbox, settings }));
}
