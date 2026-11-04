use crate::Fault;

use super::AppContext;

pub fn time_manager_request(ctx: &mut AppContext) -> Result<(), Fault> {
    ctx.platform().ok_or(Fault::host_missing())?.time_manager_request();

    Ok(())
}
