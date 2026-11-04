use crate::Fault;

use super::{AppContext, Pinch, get_battle_status, set_keep_awake, sound_manager, sound_pause_all};

pub fn app_on_focus_lost(ctx: &mut AppContext) -> Result<(), Fault> {
    sound_pause_all(sound_manager(ctx)?)?;

    if ctx.i32_at(AppContext::SCENE_ID)? == 0x12c && get_battle_status(ctx)? != 0 {
        get_battle_status(ctx)?;
    }

    set_keep_awake(ctx, 0)?;
    ctx.set_block_at::<1>(AppContext::PINCH + Pinch::ACTIVE, [0])?;
    ctx.set_block_at::<2>(AppContext::PINCH + Pinch::FIRST_DOWN, [0, 0])?;
    ctx.set_i32_at(AppContext::TOUCH_BEGAN, 0)?;
    ctx.set_block_at::<2>(AppContext::TOUCH_PENDING_RELEASED, [0, 0])
}
