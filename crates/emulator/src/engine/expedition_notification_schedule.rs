use crate::Fault;

use super::AppContext;

pub fn expedition_notification_schedule(ctx: &mut AppContext, kind: i32, flag: u8, cancel: u8) -> Result<(), Fault> {
    ctx.platform().ok_or(Fault::host_missing())?.expedition_notification_schedule(kind, flag, cancel);

    Ok(())
}
