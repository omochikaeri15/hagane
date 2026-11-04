use crate::Fault;

use super::AppContext;

pub fn au_resume_all(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.audio().ok_or(Fault::host_missing())?.resume_all();

    Ok(())
}
