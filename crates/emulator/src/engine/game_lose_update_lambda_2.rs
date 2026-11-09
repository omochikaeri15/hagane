use crate::Fault;

use super::{AppContext, dialog_close, play_sound, request_save_data};

pub fn game_lose_update_lambda_2(
    ctx: &mut AppContext,
    dialog: u64,
    event: i32,
    button: i32,
) -> Result<(), Fault> {
    if event == 2 && button == 0 {
        play_sound(ctx, 0xb, None)?;

        return dialog_close(ctx, dialog);
    }

    if event != 5 {
        return Ok(());
    }

    ctx.set_i32_at(AppContext::REWARD_POP_COUNTER, 0)?;
    ctx.roguelike.lose_tutorial_seen = true;
    request_save_data(ctx)
}
