mod arena;
mod assets;
mod device;
mod diagnostics;
mod driver;
mod gpu;
#[cfg(target_os = "ios")]
mod offscreen;
mod input;
mod keys;
mod replay;
mod session;
mod sink;
mod sound;
mod text;
mod viewport;

#[cfg(target_os = "ios")]
pub fn hardware_drawn() -> bool {
    gpu::available() || offscreen::available()
}

#[cfg(not(target_os = "ios"))]
pub fn hardware_drawn() -> bool {
    gpu::available()
}

#[cfg(target_os = "ios")]
pub fn render_offscreen(scene: ViewerScene) -> Option<Vec<u8>> {
    offscreen::render(&scene.into())
}

pub use arena::{Arena, Command, Event, Launch, Standing};
pub use diagnostics::{Diagnostics, Occupant, Vitals};
pub use driver::Label;
pub use gpu::{ViewerScene, available as gpu_available, forget as forget_scene, install as install_renderer, note_presented, submit as submit_scene};
pub use input::Touch;
pub use session::{Lag, Phase, Reel};
pub use viewport::Canvas;
