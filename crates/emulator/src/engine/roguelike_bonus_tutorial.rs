use crate::Fault;

use super::{
    AppContext, dialog_set_back_button, dialog_show, query_localizable,
    roguelike_bonus_tutorial_lambda_0,
};

pub fn roguelike_bonus_tutorial(ctx: &mut AppContext) -> Result<(), Fault> {
    if ctx.roguelike.bonus_tutorial_seen {
        return Ok(());
    }

    let text = query_localizable(ctx, b"Roguelike_TutorialText_005");
    let dialog = dialog_show(ctx, &text, 0, 0, 4, Some(roguelike_bonus_tutorial_lambda_0))?;

    dialog_set_back_button(ctx, dialog, 0)?;

    Ok(())
}
