use crate::Fault;

use super::{AppContext, sound_resume_task_start};

pub fn batch_tasks_pending(ctx: &mut AppContext) -> Result<bool, Fault> {
    if ctx.batch_tasks.is_empty() {
        return Ok(false);
    }

    let mut done;

    loop {
        done = ctx.batch_tasks.front().is_some_and(|task| task.done);

        if !done {
            break;
        }

        ctx.batch_tasks.pop_front();

        let Some(mut task) = ctx.batch_tasks.pop_front() else {
            break;
        };

        sound_resume_task_start(ctx, &mut task)?;
        ctx.batch_tasks.push_front(task);
    }

    Ok(!done)
}
