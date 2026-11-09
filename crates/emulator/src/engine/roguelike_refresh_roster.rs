use std::{cell::RefCell, rc::Rc};

use crate::Fault;

use super::{AppContext, RoguelikeProgress, roguelike_unit_level};

pub fn roguelike_refresh_roster(
    ctx: &AppContext,
    progress: &Rc<RefCell<RoguelikeProgress>>,
) -> Result<(), Fault> {
    let mut member = 0usize;

    while member < progress.borrow().roster.len() {
        let chara = progress.borrow().roster[member];
        let (level, plus, form) = roguelike_unit_level(
            ctx,
            &progress.borrow(),
            chara.unit_id,
            chara.base_level,
            chara.base_plus,
        )?;
        let mut run_progress = progress.borrow_mut();
        let chara = &mut run_progress.roster[member];

        chara.level = level;
        chara.plus_level = plus;

        if chara.top_form < form {
            chara.top_form = form;
            chara.form = form;
        }

        member += 1;
    }

    Ok(())
}
