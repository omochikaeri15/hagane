use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use slint::ComponentHandle;
use tracing::info;

use foundry::domains::cat::scanner::{self as cat_scanner, CatEntry};
use foundry::domains::enemy::scanner::{self as enemy_scanner, EnemyEntry};
use foundry::domains::settings::ScannerConfig;
use foundry::domains::stage::scanner::{self as stage_scanner, StageBundle};

use crate::AppWindow;
use crate::bridge::{self, Ticket};
use crate::engine::Engine;

pub type Listener = Rc<dyn Fn(&AppWindow)>;

#[derive(Clone)]
pub enum Load<T> {
    Idle,
    Loading(usize, usize),
    Ready(Rc<T>),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<Rc<T>> {
        match self {
            Self::Ready(data) => Some(Rc::clone(data)),
            _ => None,
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Loading(done, total) if *total > 0 => Some(format!("{}% / 100%", done * 100 / total)),
            _ => None,
        }
    }

    pub fn scanning(&self) -> bool {
        matches!(self, Self::Loading(_, total) if *total > 0)
    }
}

struct Inner {
    cats: RefCell<Load<Vec<CatEntry>>>,
    enemies: RefCell<Load<Vec<EnemyEntry>>>,
    stages: RefCell<Load<StageBundle>>,
    generation: Cell<u32>,
    listeners: RefCell<Vec<Listener>>,
    progress: Cell<Option<Ticket>>,
    landing: Cell<Option<Ticket>>,
}

#[derive(Clone)]
pub struct Catalogs(Rc<Inner>);

#[derive(Clone, Copy)]
enum Kind {
    Cats,
    Enemies,
    Stages,
}

type Progress = (u32, Kind, usize, usize);
type Arrival = (u32, Kind, Landed);

impl Catalogs {
    pub fn new() -> Self {
        let inner = Rc::new(Inner {
            cats: RefCell::new(Load::Idle),
            enemies: RefCell::new(Load::Idle),
            stages: RefCell::new(Load::Idle),
            generation: Cell::new(0),
            listeners: RefCell::new(Vec::new()),
            progress: Cell::new(None),
            landing: Cell::new(None),
        });
        let weak = Rc::downgrade(&inner);
        let progress = bridge::subscribe::<Progress>(move |ui, (round, kind, done, total)| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let catalogs = Catalogs(inner);

            if catalogs.0.generation.get() != round {
                return;
            }

            match kind {
                Kind::Cats => *catalogs.0.cats.borrow_mut() = Load::Loading(done, total),
                Kind::Enemies => *catalogs.0.enemies.borrow_mut() = Load::Loading(done, total),
                Kind::Stages => *catalogs.0.stages.borrow_mut() = Load::Loading(done, total),
            }

            catalogs.notify(ui);
        });
        let weak = Rc::downgrade(&inner);
        let landing = bridge::subscribe::<Arrival>(move |ui, (round, kind, landed)| {
            if let Some(inner) = weak.upgrade() {
                Catalogs(inner).land(ui, round, kind, landed);
            }
        });

        inner.progress.set(Some(progress));
        inner.landing.set(Some(landing));
        Self(inner)
    }

    pub fn listen(&self, listener: Listener) {
        self.0.listeners.borrow_mut().push(listener);
    }

    pub fn notify(&self, ui: &AppWindow) {
        let listeners: Vec<Listener> = self.0.listeners.borrow().iter().cloned().collect();

        for listener in listeners {
            listener(ui);
        }
    }

    pub fn cats(&self) -> Load<Vec<CatEntry>> {
        self.0.cats.borrow().clone()
    }

    pub fn enemies(&self) -> Load<Vec<EnemyEntry>> {
        self.0.enemies.borrow().clone()
    }

    pub fn stages(&self) -> Load<StageBundle> {
        self.0.stages.borrow().clone()
    }

    pub fn reset(&self, ui: &AppWindow) {
        self.0.generation.set(self.0.generation.get().wrapping_add(1));
        *self.0.cats.borrow_mut() = Load::Idle;
        *self.0.enemies.borrow_mut() = Load::Idle;
        *self.0.stages.borrow_mut() = Load::Idle;
        self.notify(ui);
    }

    pub fn load_cats(&self, window: &AppWindow, engine: &Engine, config: ScannerConfig) {
        self.start(window, engine, Kind::Cats, config.clone());
        self.start(window, engine, Kind::Stages, config);
    }

    pub fn load_enemies(&self, window: &AppWindow, engine: &Engine, config: ScannerConfig) {
        self.start(window, engine, Kind::Enemies, config);
    }

    pub fn load_stages(&self, window: &AppWindow, engine: &Engine, config: ScannerConfig) {
        self.start(window, engine, Kind::Stages, config.clone());
        self.start(window, engine, Kind::Cats, config.clone());
        self.start(window, engine, Kind::Enemies, config);
    }

    fn idle(&self, kind: &Kind) -> bool {
        match kind {
            Kind::Cats => matches!(*self.0.cats.borrow(), Load::Idle),
            Kind::Enemies => matches!(*self.0.enemies.borrow(), Load::Idle),
            Kind::Stages => matches!(*self.0.stages.borrow(), Load::Idle),
        }
    }

    fn start(&self, window: &AppWindow, engine: &Engine, kind: Kind, config: ScannerConfig) {
        if !self.idle(&kind) || !engine.shared().ready() {
            return;
        }

        let round = self.0.generation.get();
        let label = match kind {
            Kind::Cats => "cats",
            Kind::Enemies => "enemies",
            Kind::Stages => "stages",
        };

        match kind {
            Kind::Cats => *self.0.cats.borrow_mut() = Load::Loading(0, 0),
            Kind::Enemies => *self.0.enemies.borrow_mut() = Load::Loading(0, 0),
            Kind::Stages => *self.0.stages.borrow_mut() = Load::Loading(0, 0),
        }

        self.notify(window);

        let (Some(progress), Some(landing)) = (self.0.progress.get(), self.0.landing.get()) else {
            return;
        };
        let weak = window.as_weak();
        let progress_weak = window.as_weak();
        let cached = engine.shared().cached();
        let vault: Arc<_> = engine.vault();

        info!(label, cached, "Starting a scan");

        engine.run(move |_| {
            let report = move |done: usize, total: usize| bridge::post(&progress_weak, progress, (round, kind, done, total));

            let landed = match kind {
                Kind::Cats => Landed::Cats(cached.then(|| cat_scanner::hydrate(&vault)).flatten().map(|(_, cats)| cats).unwrap_or_else(|| {
                    let scan = cat_scanner::load(config, Arc::clone(&vault), report);

                    if let Some(bytes) = scan.key.and_then(|key| cat_scanner::payload(key, &scan.data)) {
                        cat_scanner::persist(&bytes);
                    }

                    scan.data
                })),
                Kind::Enemies => Landed::Enemies(cached.then(enemy_scanner::hydrate).flatten().map(|(_, enemies)| enemies).unwrap_or_else(|| {
                    let scan = enemy_scanner::load(config, Arc::clone(&vault), report);

                    if let Some(bytes) = scan.key.and_then(|key| enemy_scanner::payload(key, &scan.data)) {
                        enemy_scanner::persist(&bytes);
                    }

                    scan.data
                })),
                Kind::Stages => Landed::Stages(Box::new(cached.then(stage_scanner::hydrate).flatten().map(|(_, bundle)| bundle).unwrap_or_else(|| {
                    let scan = stage_scanner::load(config, Arc::clone(&vault), report);

                    if let Some(bytes) = scan.key.and_then(|key| stage_scanner::payload(key, &scan.data)) {
                        stage_scanner::persist(&bytes);
                    }

                    scan.data
                }))),
            };

            bridge::post(&weak, landing, (round, kind, landed));
        });
    }

    fn land(&self, ui: &AppWindow, round: u32, kind: Kind, landed: Landed) {
        if self.0.generation.get() != round {
            return;
        }

        match (kind, landed) {
            (Kind::Cats, Landed::Cats(cats)) => {
                info!(count = cats.len(), "Cats are ready");
                *self.0.cats.borrow_mut() = Load::Ready(Rc::new(cats));
            }
            (Kind::Enemies, Landed::Enemies(enemies)) => {
                info!(count = enemies.len(), "Enemies are ready");
                *self.0.enemies.borrow_mut() = Load::Ready(Rc::new(enemies));
            }
            (Kind::Stages, Landed::Stages(bundle)) => {
                info!(stages = bundle.registry.stages.len(), "Stages are ready");
                *self.0.stages.borrow_mut() = Load::Ready(Rc::new(*bundle));
            }
            _ => {}
        }

        self.notify(ui);
    }
}

enum Landed {
    Cats(Vec<CatEntry>),
    Enemies(Vec<EnemyEntry>),
    Stages(Box<StageBundle>),
}
