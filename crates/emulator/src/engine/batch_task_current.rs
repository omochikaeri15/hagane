use super::{AppContext, SoundResumeBatchTask};

pub fn batch_task_current(ctx: &AppContext) -> Option<&SoundResumeBatchTask> {
    ctx.batch_tasks.front()
}
