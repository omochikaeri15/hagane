use crate::Fault;

use super::AppContext;

pub fn adjust_track_event(ctx: &mut AppContext, token: &[u8], value: &[u8]) -> Result<(), Fault> {
    ctx.platform().ok_or(Fault::host_missing())?.adjust_track_event(token, value);

    Ok(())
}
