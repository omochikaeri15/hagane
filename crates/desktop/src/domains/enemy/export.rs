use foundry::common::context::GlobalContext;
use foundry::systems::combat::registry::Magnification;
use foundry::domains::enemy::scanner::EnemyEntry;
use foundry::domains::enemy::statblock::{self, Subject};
use foundry::domains::settings::Settings;
use foundry::Vault;

use crate::common::SpriteSheet;
use crate::widget::statblock_export::Request;

pub(super) struct Ctx<'a> {
    pub(super) enemy: &'a EnemyEntry,
    pub(super) magnification: Magnification,
    pub(super) sheets: &'a [SpriteSheet],
    pub(super) global: GlobalContext<'a>,
    pub(super) settings: &'a Settings,
    pub(super) vault: &'a Vault,
}

pub(super) fn request(ctx: Ctx<'_>) -> Option<Request<'_>> {
    let data = statblock::build(Subject {
        enemy: ctx.enemy,
        magnification: ctx.magnification,
        show_invalid: ctx.settings.show_invalid_enemies(),
        global: ctx.global,
        vault: ctx.vault,
    });

    Some(Request { data, sheets: ctx.sheets, settings: ctx.settings })
}
