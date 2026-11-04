use crate::Fault;

use super::{AppContext, SoundResumeBatchTask, sound_resume_task_run};

pub fn sound_resume_task_start(ctx: &mut AppContext, task: &mut SoundResumeBatchTask) -> Result<(), Fault> {
    sound_resume_task_run(ctx, task)
}
