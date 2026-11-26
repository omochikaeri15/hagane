use std::sync::Arc;

use codex::combat::Faction;

use crate::systems::animation::collision::Profile;
use crate::systems::animation::{self, named_offsets, Motion, MotionSet, Loop, RigFiles};
use crate::Vfs;

use super::files;
use super::scanner::EnemyEntry;

const ZOMBIE_MARK: &str = "_zombie";

pub fn set_id(enemy: &EnemyEntry) -> String {
    enemy.id_str()
}

pub fn rig_files(enemy: &EnemyEntry, vfs: &Vfs) -> Option<RigFiles> {
    let plain = files::anim_base_filename(enemy.id);
    let bases = vec![plain.clone(), format!("i{}", plain)];

    animation::rig_files(vfs, &set_id(enemy), &bases)
}

pub fn motion_name(suffix: &str) -> Option<&'static str> {
    suffix.strip_prefix(ZOMBIE_MARK).map_or_else(
        || animation::standard_name(suffix),
        |zombie| zombie.parse::<usize>().ok().and_then(zombie_name),
    )
}

pub fn motions(enemy: &EnemyEntry, vfs: &Vfs) -> MotionSet {
    let plain = files::anim_base_filename(enemy.id);
    let bases = vec![plain.clone(), format!("i{}", plain)];

    let Some(rig) = animation::rigging(vfs, &plain, &bases) else {
        return MotionSet::default();
    };

    let fighter = Arc::new(Profile::of(&enemy.stats));
    let mut motions = Vec::new();
    let mut own_soul = None;

    for (suffix, path) in animation::maanims(vfs, &bases) {
        if animation::own_soul(&suffix, Some(&enemy.stats)) {
            own_soul = Some(path);
            continue;
        }

        let zombie = suffix.strip_prefix(ZOMBIE_MARK);
        let index = zombie.unwrap_or(&suffix).parse::<usize>().ok();

        let named = if zombie.is_some() { None } else { index.and_then(animation::standard) };
        let extra = animation::special_pose(vfs, &suffix, &enemy.stats, Faction::Enemy);

        motions.push(Motion {
            name: motion_name(&suffix).map(str::to_string),
            slot: named.map(|(_, slot, _)| slot),
            role: named.map(|(_, _, role)| role),
            looping: match (named, extra) {
                (Some((_, _, role)), _) => role.looping(Faction::Enemy),
                (None, Some(pose)) => Loop::Pose(pose),
                (None, None) => Loop::Frames,
            },
            rig: rig.clone(),
            file: Some(path),
            combat: None,
        }.fought_by(Some(&fighter)));
    }

    if let Some(soul) = animation::soul(vfs, &enemy.stats, Some(&rig), own_soul) {
        motions.push(soul);
    }

    motions.push(Motion::model(rig).fought_by(Some(&fighter)));

    MotionSet { name: set_id(enemy), motions, offsets: named_offsets("Base HP") }
}

fn zombie_name(index: usize) -> Option<&'static str> {
    match index {
        0 => Some("Burrow"),
        1 => Some("Dig"),
        2 => Some("Surface"),
        _ => None,
    }
}
