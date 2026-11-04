use crate::Fault;

use super::{AppContext, SoundResumeBatchTask, sound_resume_task_new};

pub fn sound_resume_task_make(ctx: &mut AppContext, resume: bool) -> Result<SoundResumeBatchTask, Fault> {
    sound_resume_task_new(ctx, resume)
}
