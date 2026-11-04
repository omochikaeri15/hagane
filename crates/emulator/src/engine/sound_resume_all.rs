use crate::Fault;

use super::{AppContext, play_audio, sound_manager};

pub fn sound_resume_all(ctx: &mut AppContext) -> Result<(), Fault> {
    sound_manager(ctx)?.paused = 0;

    let mut sound = 0i64;

    while sound < sound_manager(ctx)?.count as i64 {
        if sound_manager(ctx)?.looping.get(sound as usize).copied().unwrap_or(false) {
            play_audio(ctx, sound as i32, None, true)?;
        }

        sound += 1;
    }

    Ok(())
}
