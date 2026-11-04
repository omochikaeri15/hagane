use crate::Fault;

use super::{AppContext, SoundResumeBatchTask, sound_resume_task_start};

pub fn batch_task_enqueue(ctx: &mut AppContext, mut task: SoundResumeBatchTask) -> Result<(), Fault> {
    if ctx.batch_tasks.is_empty() {
        sound_resume_task_start(ctx, &mut task)?;
    }

    ctx.batch_tasks.push_back(task);

    Ok(())
}
