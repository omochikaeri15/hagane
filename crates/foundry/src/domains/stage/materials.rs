use codex::chapter::map::DropItemEntry;

use crate::{common::decimal::trimmed, ItemStore, Vfs};

use super::files;
use super::treasure::ResolvedDrop;
use super::Stage;

pub const MAT_IDS: [u32; 16] = [
    85, 86, 87, 88, 89, 90, 91, 140,
    187, 188, 189, 190, 191, 192, 193, 194,
];

pub fn heading(drops: &DropItemEntry, stage: &Stage, crown: u8) -> String {
    drops.roll(stage.stage_id as usize, usize::from(crown)).map_or_else(
        || "Materials".to_owned(),
        |roll| format!("Materials | Amount: {} ({}\u{d7}{:.2})", roll.rolls, roll.rate, roll.multiplier),
    )
}

pub fn odds(drops: &DropItemEntry, index: usize) -> String {
    format!("{}%", trimmed(drops.odds(index)))
}

pub(crate) fn shown_odds(drops: &DropItemEntry, index: usize) -> f64 {
    let exact = drops.odds(index);

    trimmed(exact).parse().unwrap_or(exact)
}

pub fn weight(drops: &DropItemEntry, index: usize) -> String {
    let raw = format!("Raw weight {} of {}", drops.material_drops.get(index).copied().unwrap_or(0), drops.weight_total());

    if drops.dud_chance == 0 {
        raw
    } else {
        format!("{raw}\n{}% of rolls drop nothing", drops.dud_chance.min(100))
    }
}

pub fn resolve(vfs: &Vfs, idx: usize, amt: u32, items: &ItemStore) -> ResolvedDrop {
    let Some(&item_id) = MAT_IDS.get(idx) else {
        return ResolvedDrop {
            name: format!("ID {}", idx),
            image_path: None,
            amount_display: amt.to_string(),
        };
    };

    ResolvedDrop {
        name: items.name(vfs, item_id).unwrap_or_else(|| format!("ID {}", item_id)),
        image_path: items
            .icon_index(vfs, item_id)
            .and_then(|index| vfs.find(&files::gatya_item_img(index))),
        amount_display: amt.to_string(),
    }
}