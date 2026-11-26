use std::sync::Arc;

use codex::combat::Faction;
use codex::graphics::rig::Pose;

use crate::systems::animation::collision::{Combatant, Profile};
use crate::systems::animation::{self, named_offsets, Motion, MotionSet, Loop, RigFiles};
use crate::Vfs;

use super::files;
use super::scanner::CatEntry;
use super::waiter::unitid;

const SPIRIT_ATTACK: usize = 2;

pub fn set_id(cat: &CatEntry, form: usize) -> String {
    let form_char = match form {
        0 => 'f',
        1 => 'c',
        2 => 's',
        _ => 'u',
    };

    format!("{:03}_{}", cat.id, form_char)
}

pub fn rig_files(cat: &CatEntry, form: usize, vfs: &Vfs) -> Option<RigFiles> {
    let egg_ids = cat.egg_ids.unwrap_or((-1, -1));
    let base = vec![files::anim_base_filename(cat.id, form, egg_ids)];

    animation::rig_files(vfs, &set_id(cat, form), &base)
}

pub fn motion_name(suffix: &str) -> Option<&'static str> {
    animation::standard_name(suffix)
}

pub fn motions(cat: &CatEntry, form: usize, vfs: &Vfs) -> MotionSet {
    let egg_ids = cat.egg_ids.unwrap_or((-1, -1));
    let base = vec![files::anim_base_filename(cat.id, form, egg_ids)];
    let id = set_id(cat, form);

    let stats = cat.stats.get(form).and_then(Option::as_ref);
    let fighter = stats.map(|stats| Arc::new(Profile::of(stats)));
    let mut motions = Vec::new();
    let rig = animation::rigging(vfs, &id, &base);
    let mut own_soul = None;

    if let Some(rig) = rig.as_ref() {
        for (suffix, path) in animation::maanims(vfs, &base) {
            if animation::own_soul(&suffix, stats) {
                own_soul = Some(path);
                continue;
            }

            let index = suffix.parse::<usize>().ok();
            let named = index.and_then(animation::standard);
            let extra = stats.and_then(|stats| animation::special_pose(vfs, &suffix, stats, Faction::Cat));

            motions.push(Motion {
                name: motion_name(&suffix).map(str::to_string),
                slot: named.map(|(_, slot, _)| slot),
                role: named.map(|(_, _, role)| role),
                looping: match (named, extra) {
                    (Some((_, _, role)), _) => role.looping(Faction::Cat),
                    (None, Some(pose)) => Loop::Pose(pose),
                    (None, None) => Loop::Frames,
                },
                rig: rig.clone(),
                file: Some(path),
                combat: None,
            }.fought_by(fighter.as_ref()));
        }

        motions.push(Motion::model(Arc::clone(rig)).fought_by(fighter.as_ref()));
    }

    if let Some(spirit) = spirit_motion(cat, form, vfs) {
        motions.push(spirit);
    }

    if let Some(soul) = stats.and_then(|stats| animation::soul(vfs, stats, rig.as_ref(), own_soul)) {
        motions.push(soul);
    }

    MotionSet { name: id, motions, offsets: named_offsets("Gacha") }
}

fn spirit_motion(cat: &CatEntry, form: usize, vfs: &Vfs) -> Option<Motion> {
    let conjure_id = cat.stats.get(form)?.as_ref()?.conjure_unit_id;

    if conjure_id <= 0 {
        return None;
    }

    let spirit_id = conjure_id as u32;
    let base = vec![files::anim_base_filename(spirit_id, 0, (-1, -1))];
    let rig = animation::rigging(vfs, &format!("spirit_{}", spirit_id), &base)?;
    let anim = vfs.find(&files::maanim_file(spirit_id, 0, (-1, -1), SPIRIT_ATTACK)).map(|path| vfs.source(&path))?;
    let combat = unitid(vfs, conjure_id)
        .and_then(|forms| forms.into_iter().next())
        .map(|stats| Combatant { profile: Arc::new(Profile::of(&stats)), attacking: true });

    Some(Motion {
        name: Some("Spirit".to_string()),
        slot: None,
        role: None,
        looping: Loop::Pose(Pose::Attack(Faction::Cat)),
        rig,
        file: Some(anim),
        combat,
    })
}
