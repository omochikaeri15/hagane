use crate::Fault;

use super::{AppContext, app_on_focus_gained, app_on_focus_lost, gl_surface_set_focused};

pub fn app_focus_changed(ctx: &mut AppContext, focused: i32) -> Result<(), Fault> {
    if focused == 0 {
        gl_surface_set_focused(ctx, false);

        return app_on_focus_lost(ctx);
    }

    gl_surface_set_focused(ctx, true);

    app_on_focus_gained(ctx)
}
