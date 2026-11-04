use crate::Fault;

use super::{AppContext, SoundResumeBatchTask, au_resume_all, play_audio, sound_load, sound_manager};

pub fn sound_resume_task_run(ctx: &mut AppContext, task: &mut SoundResumeBatchTask) -> Result<(), Fault> {
    au_resume_all(ctx)?;

    if sound_manager(ctx)?.count > 0 {
        let mut sound = 0i32;

        loop {
            sound_load(ctx, sound)?;

            let count = sound_manager(ctx)?.count;

            task.progress = sound as f32 / count as f32;
            sound = sound.wrapping_add(1);

            if sound >= count {
                break;
            }
        }
    }

    if task.resume && sound_manager(ctx)?.paused != 0 {
        sound_manager(ctx)?.paused = 0;

        let mut sound = 0i64;

        while sound < sound_manager(ctx)?.count as i64 {
            if sound_manager(ctx)?.looping.get(sound as usize).copied().unwrap_or(false) {
                play_audio(ctx, sound as i32, None, true)?;
            }

            sound += 1;
        }
    }

    task.done = true;

    Ok(())
}
