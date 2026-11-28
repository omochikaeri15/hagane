use codex::combat::{Foe, Terms};

use crate::common::context::GlobalContext;
use crate::domains::cat::files;
use crate::systems::combat::lens::{self, Lens, Pace};
use crate::systems::combat::registry::{format_stat, Magnification, StatContext, STAT_ATK_CYCLE, STAT_ATTACK, STAT_CASH_DROP, STAT_DPS, STAT_HITPOINTS, STAT_KNOCKBACKS, STAT_RANGE, STAT_SPEED};
use crate::systems::combat::RenderContext;
use crate::systems::statblock::{DetailsData, SheetHead, StatCell, StatblockData};
use crate::Vault;

use super::scanner::{self, EnemyEntry};

pub struct Subject<'a> {
    pub enemy: &'a EnemyEntry,
    pub magnification: Magnification,
    pub show_invalid: bool,
    pub global: GlobalContext<'a>,
    pub vault: &'a Vault,
    pub lens: Lens,
}

const NO_DESCRIPTION: &str = "No description available";

pub fn details(enemy: &EnemyEntry, global: GlobalContext<'_>) -> DetailsData {
    let mut description: Vec<String> = enemy.description.iter().map(|line| line.trim_end().to_owned()).collect();

    while description.last().is_some_and(String::is_empty) {
        description.pop();
    }

    if description.is_empty() {
        description.push(NO_DESCRIPTION.to_owned());
    }

    DetailsData {
        head: SheetHead {
            id_str: enemy.id_str(),
            name: enemy.display_name(),
            icon: files::portrait(&global.vault.vfs, enemy.icon_path.as_ref()),
            top: None,
        },
        description,
        evolve: None,
        combos: Vec::new(),
    }
}

pub fn build(subject: Subject<'_>) -> StatblockData {
    let enemy = subject.enemy;
    let dynamic_entry = scanner::scan_single(enemy.id, subject.vault, subject.show_invalid);
    let stats = dynamic_entry.as_ref().map_or(&enemy.stats, |entry| &entry.stats);

    let ctx = RenderContext::enemy(subject.global, stats, subject.magnification);

    assemble(&ctx, enemy, lens::fitted(stats, subject.lens))
}

fn assemble(ctx: &RenderContext<'_>, enemy_entry: &EnemyEntry, lens: Lens) -> StatblockData {
    let stat_ctx = StatContext::enemy(ctx.final_stats, enemy_entry.atk_anim_frames, ctx.magnification, ctx.global.treasure);
    let cycle = (STAT_ATK_CYCLE.get_value)(&stat_ctx);
    let (traits, h1, h2, b1, b2, footer) = lens::plain(ctx, Pace::of(&stat_ctx), lens);
    let measured = lens::measure(&stat_ctx, &lens::rules(&ctx.global), Foe::default(), Terms::default(), lens);

    let (hitpoints, attack) = ctx.magnification.shown(0);
    let top_val_str = if ctx.magnification.split() { format!("{hitpoints}%/{attack}%") } else { format!("{hitpoints}%") };

    let headers_1 = vec![
        STAT_ATTACK.display_name.to_string(),
        STAT_DPS.display_name.to_string(),
        STAT_RANGE.display_name.to_string(),
        STAT_ATK_CYCLE.display_name.to_string(),
    ];

    let data_1 = vec![
        StatCell::Text(lens::rate(measured.attack)),
        StatCell::Text(lens::rate(measured.dps)),
        StatCell::Text(format_stat(&STAT_RANGE, &stat_ctx)),
        StatCell::Frames(cycle),
    ];

    let headers_2 = vec![
        STAT_HITPOINTS.display_name.to_string(),
        STAT_KNOCKBACKS.display_name.to_string(),
        STAT_SPEED.display_name.to_string(),
        STAT_CASH_DROP.display_name.to_string(),
    ];

    let data_2 = vec![
        StatCell::Text(format_stat(&STAT_HITPOINTS, &stat_ctx)),
        StatCell::Text(format_stat(&STAT_KNOCKBACKS, &stat_ctx)),
        StatCell::Text(format_stat(&STAT_SPEED, &stat_ctx)),
        StatCell::Text(format_stat(&STAT_CASH_DROP, &stat_ctx)),
    ];

    StatblockData {
        is_cat: false,
        id_str: enemy_entry.id_str(),
        name: enemy_entry.display_name(),
        icon: files::portrait(&ctx.global.vault.vfs, enemy_entry.icon_path.as_ref()),
        top_label: "Magnify:".to_string(),
        top_value: top_val_str,
        headers_1,
        data_1,
        headers_2,
        data_2,
        marks_1: Vec::new(),
        marks_2: Vec::new(),
        traits, h1, h2, b1, b2, footer, spirit_data: None,
        tag: lens::tag(ctx.final_stats, lens),
        orbs: Vec::new(),
    }
}
