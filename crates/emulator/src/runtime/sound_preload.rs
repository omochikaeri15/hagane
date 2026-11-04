use crate::Fault;
use crate::engine::{AppContext, sound_load};

pub fn preload_sounds(ctx: &mut AppContext) -> Result<(), Fault> {
    let mut sound_id = 0;

    while sound_id < ctx.sound_manager.count {
        sound_load(ctx, sound_id)?;
        sound_id += 1;
    }

    ctx.set_block_at::<1>(AppContext::SOUNDS_LOADED, [1])
}
