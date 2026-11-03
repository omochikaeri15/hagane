mod android;
mod bridge;
mod catalog;
mod context;
mod delivery;
mod engine;
mod host;
mod images;
mod ios;
mod logging;
mod memory;
mod pages;
pub mod paths;
mod picker;
mod platform;
mod roster;
mod settings;
mod update;
mod viewer;

use std::rc::Rc;

use slint::{ComponentHandle, PlatformError};
use tracing::{debug, info};

use foundry::common::architecture;

use paths::Home as DataHome;

slint::include_modules!();

pub fn run(home: DataHome) -> Result<(), PlatformError> {
    if !home.anchor() {
        return Err(PlatformError::Other("the data folder could not be prepared".to_owned()));
    }

    let saved = settings::Saved::load();

    logging::init(saved.settings.general.enable_logging);
    info!(version = env!("CARGO_PKG_VERSION"), "Battle Cats Complete mobile is starting");
    architecture::work_cleanup();
    picker::sweep();

    let window = AppWindow::new()?;
    let state = Rc::new(settings::State::new(saved));

    #[cfg(target_os = "android")]
    {
        window.window().set_fullscreen(true);
        platform::immersive();
        android::watch_window(&window);
    }

    platform::set_orientation(state.orientation());
    let memory = Rc::new(memory::Memory::load());

    window.global::<Shell>().set_game_empty(!architecture::game_present());
    window.global::<Shell>().on_measured(|width, height, top, bottom| debug!(width, height, top, bottom, "Window geometry"));

    let engine = engine::Engine::start(&window, &state, &memory);
    let context = pages::Context::new(&window, state, memory, engine);

    pages::wire(&window, &context);

    window.run()
}
