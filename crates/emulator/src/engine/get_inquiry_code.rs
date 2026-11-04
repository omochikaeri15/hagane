use crate::Fault;

use super::AppContext;

pub fn get_inquiry_code(ctx: &mut AppContext) -> Result<Vec<u8>, Fault> {
    Ok(ctx.platform().ok_or(Fault::host_missing())?.inquiry_code())
}
