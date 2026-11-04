use super::AppContext;

pub fn gl_surface_set_focused(ctx: &mut AppContext, focused: bool) {
    ctx.surface_lost = !focused;
}
