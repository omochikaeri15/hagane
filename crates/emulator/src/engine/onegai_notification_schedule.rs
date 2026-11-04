use crate::Fault;

use super::AppContext;

pub fn onegai_notification_schedule(ctx: &mut AppContext, flag: u8, cancel: u8) -> Result<(), Fault> {
    ctx.platform().ok_or(Fault::host_missing())?.onegai_notification_schedule(flag, cancel);

    Ok(())
}
