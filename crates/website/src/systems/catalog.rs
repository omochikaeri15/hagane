use std::rc::Rc;

use dioxus::core::spawn_forever;
use dioxus::prelude::*;

use foundry::domains::cat::scanner::CatEntry;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::settings::ScannerConfig;
use foundry::domains::stage::scanner::StageBundle;

use crate::platform::{Fault, Pending, Platform, Progress, Report};

#[derive(Clone)]
pub(crate) enum Load<T> {
    Idle,
    Loading(Progress),
    Ready(Rc<T>),
    Failed(Fault),
}

#[derive(Clone, Copy)]
pub(crate) struct Catalogs {
    cats: Signal<Load<Vec<CatEntry>>>,
    enemies: Signal<Load<Vec<EnemyEntry>>>,
    stages: Signal<Load<StageBundle>>,
    generation: Signal<u32>,
}

pub(crate) fn provide() -> Catalogs {
    use_context_provider(|| Catalogs { cats: Signal::new(Load::Idle), enemies: Signal::new(Load::Idle), stages: Signal::new(Load::Idle), generation: Signal::new(0) })
}

pub(crate) fn catalogs() -> Catalogs {
    use_context::<Catalogs>()
}

fn ahead(progress: Progress, held: Progress) -> bool {
    match (progress.scanning, held.scanning) {
        (true, false) => true,
        (false, true) => false,
        _ => progress.done >= held.done,
    }
}

fn start<T: 'static>(mut slot: Signal<Load<T>>, generation: Signal<u32>, pending: impl FnOnce(Report) -> Pending<Result<T, Fault>>, then: impl FnOnce() + 'static) {
    let (ready, loading) = {
        let held = slot.peek();

        (matches!(&*held, Load::Ready(_)), matches!(&*held, Load::Loading(_)))
    };

    if ready {
        then();
        return;
    }

    if loading {
        return;
    }

    let round = *generation.peek();

    slot.set(Load::Loading(Progress::default()));

    let report: Report = Box::new(move |progress| {
        let mut slot = slot;

        if *generation.peek() != round {
            return;
        }

        if let Load::Loading(held) = &mut *slot.write()
            && ahead(progress, *held)
        {
            *held = progress;
        }
    });
    let outcome = pending(report);

    spawn_forever(async move {
        let landed = match outcome.await {
            Ok(data) => Load::Ready(Rc::new(data)),
            Err(fault) => Load::Failed(fault),
        };

        if *generation.peek() == round {
            slot.set(landed);
            then();
        }
    });
}

impl Catalogs {
    pub(crate) fn cats(&self) -> Load<Vec<CatEntry>> {
        (self.cats)()
    }

    pub(crate) fn enemies(&self) -> Load<Vec<EnemyEntry>> {
        (self.enemies)()
    }

    pub(crate) fn stages(&self) -> Load<StageBundle> {
        (self.stages)()
    }

    pub(crate) fn load_cats(&self, platform: &Platform, config: ScannerConfig) {
        let (stages, generation, follower, companion) = (self.stages, self.generation, platform.clone(), config.clone());

        start(self.cats, self.generation, |report| platform.host().cats(config, report), move || {
            start(stages, generation, |report| follower.host().stages(companion, report), || {});
        });
    }

    pub(crate) fn load_enemies(&self, platform: &Platform, config: ScannerConfig) {
        start(self.enemies, self.generation, |report| platform.host().enemies(config, report), || {});
    }

    pub(crate) fn load_stages(&self, platform: &Platform, config: ScannerConfig) {
        let (cats, generation, follower, companion) = (self.cats, self.generation, platform.clone(), config.clone());

        start(self.stages, self.generation, |report| platform.host().stages(config, report), move || {
            start(cats, generation, |report| follower.host().cats(companion, report), || {});
        });
    }

    pub(crate) fn reset(&mut self) {
        let next = self.generation.peek().wrapping_add(1);

        self.generation.set(next);
        self.cats.set(Load::Idle);
        self.enemies.set(Load::Idle);
        self.stages.set(Load::Idle);
    }
}
