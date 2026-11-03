use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread;
use std::time::Instant;

use slint::ComponentHandle;
use tracing::{error, info, warn};

use foundry::common::architecture;
use foundry::domains::import;
use foundry::domains::mods;
use foundry::domains::settings::ScannerConfig;
use foundry::{ContentStore, Vault};

use crate::bridge;
use crate::memory::Memory;
use crate::settings::State;
use crate::{AppWindow, Shell};

const WALK_THREADS: usize = 2;
const STARTING: &str = "Starting the data engine...";
const HYDRATING: &str = "Reading the cached index...";
const WALKING: &str = "Indexing game files...";

pub struct Shared {
    vault: RwLock<Arc<Vault>>,
    mounted: Mutex<Option<String>>,
    cached: AtomicBool,
    ready: AtomicBool,
    generation: AtomicU64,
}

type Job = Box<dyn FnOnce(&Shared) + Send>;

pub struct Engine {
    shared: Arc<Shared>,
    jobs: Sender<Job>,
}

impl Shared {
    pub fn vault(&self) -> Arc<Vault> {
        self.vault.read().map_or_else(|poisoned| Arc::clone(&poisoned.into_inner()), |held| Arc::clone(&held))
    }

    pub fn mounted(&self) -> Option<String> {
        self.mounted.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub fn cached(&self) -> bool {
        self.cached.load(Ordering::Acquire)
    }

    pub fn uncache(&self) {
        self.cached.store(false, Ordering::Release);
    }

    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    fn adopt(&self, vault: Vault, cached: bool) {
        match self.vault.write() {
            Ok(mut held) => *held = Arc::new(vault),
            Err(poisoned) => *poisoned.into_inner() = Arc::new(vault),
        }

        self.cached.store(cached, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.ready.store(true, Ordering::Release);
    }

    fn set_mounted(&self, name: Option<String>) {
        *self.mounted.lock().unwrap_or_else(PoisonError::into_inner) = name;
    }
}

fn phase(window: &AppWindow, label: &str, ready: bool) {
    let shell = window.global::<Shell>();

    shell.set_engine_phase(label.into());
    shell.set_engine_ready(ready);
    shell.set_game_empty(!architecture::game_present());
}

fn secluded<T: Send>(job: impl FnOnce() -> T + Send) -> T {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(WALK_THREADS).thread_name(|worker| format!("vault_walk_{worker}")).build();

    match pool {
        Ok(pool) => pool.install(job),
        Err(err) => {
            warn!("The file index walk shares the global thread pool: {}", err);
            job()
        }
    }
}

fn hydrate(vault: &mut Vault) -> Option<u64> {
    let stored = vault.vfs.hydrate();

    if let Some(content) = ContentStore::hydrate() {
        content.apply(&mut vault.vds);
    }

    stored
}

pub fn mount_game(vault: &Vault) {
    match vault.vfs.create(std::path::Path::new(architecture::GAME)) {
        Ok(conflicts) => {
            for conflict in &conflicts {
                warn!(key = %conflict.key, "duplicate filename in game data, all copies excluded: {:?}", conflict.paths);
            }
        }
        Err(err) => error!("Failed to index game data: {}", err),
    }
}

pub fn mount_mod(vault: &Vault, name: &str) -> Result<(), String> {
    match mods::enable(vault, name) {
        Ok(conflicts) => {
            for conflict in &conflicts {
                warn!(key = %conflict.key, "duplicate filename in mod, all copies excluded: {:?}", conflict.paths);
            }

            Ok(())
        }
        Err(err) => {
            error!(mod_name = name, "Failed to mount the mod: {}", err);
            Err(err.to_string())
        }
    }
}

fn walk(config: &ScannerConfig) -> Vault {
    let vault = Vault::with_priority(&config.language_priority);

    mount_game(&vault);

    if let Some(name) = config.active_mod.as_deref()
        && mount_mod(&vault, name).is_err()
    {
        mods::disable(&vault, name);
    }

    vault
}

fn adopt_walk(vault: &Vault, index: u64) {
    if vault.vfs.count(architecture::GAME) == 0 {
        warn!("No game data on disk, purging every derived cache");
        import::purge_derived_caches();
    }

    vault.vfs.persist(index);
}

impl Engine {
    pub fn start(window: &AppWindow, state: &Rc<State>, memory: &Rc<Memory>) -> Rc<Self> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let mounted = memory.mounted();
        let shared = Arc::new(Shared {
            vault: RwLock::new(Arc::new(Vault::with_priority(&state.settings().general.language_priority))),
            mounted: Mutex::new(mounted.clone()),
            cached: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            generation: AtomicU64::new(0),
        });
        let worker = Arc::clone(&shared);
        let spawned = thread::Builder::new().name("engine".to_owned()).spawn(move || {
            while let Ok(job) = inbox.recv() {
                job(&worker);
            }
        });

        if let Err(err) = spawned {
            error!("The engine thread could not be started: {err}");
        }

        let engine = Rc::new(Self { shared, jobs });

        phase(window, STARTING, false);
        engine.boot(window, state.settings().scanner_config(mounted));

        engine
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn vault(&self) -> Arc<Vault> {
        self.shared.vault()
    }

    pub fn mounted(&self) -> Option<String> {
        self.shared.mounted()
    }

    pub fn run(&self, job: impl FnOnce(&Shared) + Send + 'static) {
        if self.jobs.send(Box::new(job)).is_err() {
            error!("The engine thread is gone, a job was dropped");
        }
    }

    pub fn boot(&self, window: &AppWindow, config: ScannerConfig) {
        let weak = window.as_weak();

        self.run(move |shared| {
            let started = Instant::now();

            bridge::on_ui(&weak, |ui| phase(&ui, HYDRATING, false));

            let mut cached = Vault::with_priority(&config.language_priority);
            let stored = hydrate(&mut cached);

            if stored.is_some() {
                info!(ms = started.elapsed().as_millis(), "Cached file index hydrated");
                shared.adopt(cached, true);
                bridge::on_ui(&weak, |ui| phase(&ui, WALKING, true));
            } else {
                bridge::on_ui(&weak, |ui| phase(&ui, WALKING, false));
            }

            let walking = Instant::now();
            let rebuilt = secluded(|| walk(&config));
            let index = rebuilt.hash(config.active_mod.as_deref());

            info!(ms = walking.elapsed().as_millis(), files = rebuilt.vfs.count(architecture::GAME), "File index walk finished");

            if stored == Some(index) {
                shared.cached.store(true, Ordering::Release);
            } else {
                adopt_walk(&rebuilt, index);
                shared.adopt(rebuilt, false);
            }

            bridge::on_ui(&weak, |ui| {
                phase(&ui, WALKING, true);
                ui.global::<Shell>().set_engine_generation(ui.global::<Shell>().get_engine_generation() + 1);
            });
        });
    }

    pub fn rebuild(&self, window: &AppWindow, config: ScannerConfig) {
        let weak = window.as_weak();

        self.shared.ready.store(false, Ordering::Release);
        phase(window, WALKING, false);
        self.shared.set_mounted(config.active_mod.clone());

        self.run(move |shared| {
            let rebuilt = secluded(|| walk(&config));
            let index = rebuilt.hash(config.active_mod.as_deref());

            adopt_walk(&rebuilt, index);
            shared.adopt(rebuilt, false);
            bridge::on_ui(&weak, |ui| {
                phase(&ui, WALKING, true);
                ui.global::<Shell>().set_engine_generation(ui.global::<Shell>().get_engine_generation() + 1);
            });
        });
    }

    pub fn set_mounted(&self, name: Option<String>) {
        self.shared.set_mounted(name);
    }
}
