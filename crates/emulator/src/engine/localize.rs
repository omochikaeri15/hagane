use crate::Fault;

use super::AppContext;

pub fn localize(ctx: &mut AppContext, text: &[u8]) -> Result<Vec<u8>, Fault> {
    Ok(ctx.platform().ok_or(Fault::host_missing())?.localize(text))
}
