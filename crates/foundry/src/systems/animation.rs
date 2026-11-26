pub mod alpha;
pub mod authoring;
pub mod collision;
#[cfg(feature = "export")]
pub mod export;
pub mod overlay;
pub mod paint;
pub mod posing;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

use codex::combat::{Entity, Faction};
use codex::graphics::rig::{Animation, Counter, Pose};
use serde::{Deserialize, Serialize};

use crate::{Source, Vfs};

const LOOP_CEILING: i32 = 999_999;

const MAANIM_EXT: &str = ".maanim";
const ENTRY_SUFFIX: &str = "_entry";
const SOUL_SUFFIX: &str = "_soul";
const BURROW_SUFFIX: &str = "_zombie00";
const DIG_SUFFIX: &str = "_zombie01";
const SURFACE_SUFFIX: &str = "_zombie02";
const ENTRY_EFFECT: &str = "battle_entry";
const SOUL_EFFECT: &str = "battle_soul";

const SLOT_WALK: usize = 0;
const SLOT_IDLE: usize = 1;
const SLOT_ATTACK: usize = 2;
const SLOT_KNOCKBACK: usize = 3;
pub const SLOT_MODEL: usize = 7;

const MODEL_NAME: &str = "Model";
pub(crate) const SOUL_NAME: &str = "Soul";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Role {
    Walk,
    Idle,
    Attack,
    Knockback,
}

impl Role {
    pub fn loops(self) -> bool {
        matches!(self, Role::Walk | Role::Idle)
    }

    pub fn pose(self, faction: Faction) -> Pose {
        match self {
            Role::Walk => Pose::Walk,
            Role::Idle => Pose::Idle,
            Role::Attack => Pose::Attack(faction),
            Role::Knockback => Pose::Knockback,
        }
    }

    pub fn looping(self, faction: Faction) -> Loop {
        Loop::Pose(self.pose(faction))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Loop {
    Pose(Pose),
    Frames,
    Auto,
    Continuous,
}

impl Loop {
    pub fn native(self, anim: Option<&Animation>) -> bool {
        match self {
            Loop::Pose(pose) => matches!(pose, Pose::Walk | Pose::Idle | Pose::Dig),
            Loop::Auto => anim.is_some_and(|anim| cycle(anim).is_some()),
            Loop::Frames | Loop::Continuous => false,
        }
    }

    pub fn first(self, anim: &Animation) -> i32 {
        match self {
            Loop::Pose(pose) => turn(pose, anim).0,
            _ => 0,
        }
    }

    pub fn bound(self, anim: &Animation) -> Option<i32> {
        match self {
            Loop::Pose(pose) => turn(pose, anim).1.map(|end| end - 1),
            Loop::Frames => Some(anim.declared_frames() - 1),
            Loop::Auto => restart(anim).map(|frames| frames - 1),
            Loop::Continuous => None,
        }
    }

    pub fn wraps(self, anim: &Animation) -> Option<i32> {
        match self {
            Loop::Pose(pose) => turn(pose, anim).1,
            Loop::Frames => Some(playback_frames(anim)),
            Loop::Auto => restart(anim),
            Loop::Continuous => None,
        }
    }

    pub fn seat(self, anim: &Animation, frame: f32) -> f32 {
        let first = self.first(anim);

        self.wraps(anim).map_or(frame, |end| match frame {
            frame if frame >= 0.0 && frame < end as f32 => frame,
            frame => first as f32 + seat(end - first, frame - first as f32),
        })
    }

    pub fn frames(self, anim: &Animation) -> i32 {
        match self {
            Loop::Pose(pose) => turn(pose, anim).1.unwrap_or_else(|| anim.declared_frames()),
            _ => anim.declared_frames(),
        }
    }

    pub fn period(self, anim: &Animation) -> i32 {
        match self {
            Loop::Pose(pose) => match turn(pose, anim) {
                (first, Some(end)) => end - first,
                (_, None) => anim.declared_frames(),
            },
            _ => playback_frames(anim),
        }
    }

    pub fn exported(self, anim: &Animation, frame: f32) -> f32 {
        match self {
            Loop::Pose(_) => self.seat(anim, frame),
            _ if self.native(Some(anim)) => frame,
            _ => loop_frame(anim, frame),
        }
    }
}

fn turn(pose: Pose, anim: &Animation) -> (i32, Option<i32>) {
    match anim.counter(pose) {
        Counter::Rising => (0, anim.period().filter(|frames| *frames <= LOOP_CEILING)),
        Counter::Wrapping { from, length } => match i32::try_from(length) {
            Ok(end) => (from.min(end - 1), Some(end)),
            Err(_) => (from, None),
        },
        Counter::Faulting => (0, Some(1)),
    }
}

pub struct Rigging {
    pub id: String,
    pub png: Source,
    pub cut: Source,
    pub model: Source,
}

pub struct Motion {
    pub name: Option<String>,
    pub slot: Option<usize>,
    pub role: Option<Role>,
    pub looping: Loop,
    pub rig: Arc<Rigging>,
    pub file: Option<Source>,
    pub combat: Option<collision::Combatant>,
}

#[derive(Default)]
pub struct MotionSet {
    pub name: String,
    pub motions: Vec<Motion>,
    pub offsets: Vec<Offset>,
}

pub struct Offset {
    pub row: Option<usize>,
    pub name: &'static str,
}

impl Offset {
    pub fn none(name: &'static str) -> Self {
        Self { row: None, name }
    }

    pub fn at(row: usize, name: &'static str) -> Self {
        Self { row: Some(row), name }
    }
}

pub const RAW_OFFSET: &str = "Raw";
pub const NO_OFFSET: &str = "None";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    Bare,
    Row(usize),
}

impl Default for Placement {
    fn default() -> Self {
        Placement::Row(0)
    }
}

impl Placement {
    pub fn offset(self, rows: usize) -> Option<usize> {
        match self {
            Placement::Bare => None,
            Placement::Row(row) if row < rows => Some(row),
            Placement::Row(_) => (rows > 0).then_some(0),
        }
    }
}

const COMBAT_OFFSET: &str = "Combat";

pub fn named_offsets(placement: &'static str) -> Vec<Offset> {
    vec![Offset::none(RAW_OFFSET), Offset::at(0, COMBAT_OFFSET), Offset::at(1, placement)]
}

impl Motion {
    pub fn model(rig: Arc<Rigging>) -> Self {
        Self {
            name: Some(MODEL_NAME.to_string()),
            slot: Some(SLOT_MODEL),
            role: None,
            looping: Loop::Frames,
            rig,
            file: None,
            combat: None,
        }
    }

    pub fn fought_by(mut self, profile: Option<&Arc<collision::Profile>>) -> Self {
        let attacking = self.role == Some(Role::Attack);

        self.combat = profile.map(|profile| collision::Combatant { profile: Arc::clone(profile), attacking });
        self
    }

    pub fn label(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }

        self.file
            .as_ref()
            .and_then(|file| file.path.file_stem())
            .and_then(OsStr::to_str)
            .map_or_else(|| self.rig.id.clone(), str::to_string)
    }

    pub fn slug(&self) -> String {
        let Some(name) = &self.name else {
            return self.label();
        };

        name.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect()
    }
}

pub(crate) fn standard(index: usize) -> Option<(&'static str, usize, Role)> {
    match index {
        0 => Some(("Walk", SLOT_WALK, Role::Walk)),
        1 => Some(("Idle", SLOT_IDLE, Role::Idle)),
        2 => Some(("Attack", SLOT_ATTACK, Role::Attack)),
        3 => Some(("Knockback", SLOT_KNOCKBACK, Role::Knockback)),
        _ => None,
    }
}

pub fn special_pose(vfs: &Vfs, suffix: &str, stats: &Entity, faction: Faction) -> Option<Pose> {
    let zombie = faction == Faction::Enemy && stats.trait_zombie != 0;

    match suffix {
        ENTRY_SUFFIX if stats.spawn_animation_flag != 0 => {
            Some(Pose::Entry { effect: effect_length(vfs, ENTRY_EFFECT, stats.spawn_animation_type) })
        }
        SOUL_SUFFIX if stats.use_gudetama_soul != 0 => {
            Some(Pose::Soul { effect: effect_length(vfs, SOUL_EFFECT, stats.soul_animation_type) })
        }
        BURROW_SUFFIX if zombie => Some(Pose::Burrow),
        DIG_SUFFIX if zombie => Some(Pose::Dig),
        SURFACE_SUFFIX if zombie => Some(Pose::Surface),
        _ => None,
    }
}

pub(crate) fn own_soul(suffix: &str, stats: Option<&Entity>) -> bool {
    suffix == SOUL_SUFFIX && stats.is_some_and(|stats| stats.use_gudetama_soul != 0)
}

pub(crate) fn soul(vfs: &Vfs, stats: &Entity, rig: Option<&Arc<Rigging>>, own: Option<Source>) -> Option<Motion> {
    let (rig, file, effect) = match stats.use_gudetama_soul {
        0 if stats.soul_animation_type > 0 => {
            let base = format!("{SOUL_EFFECT}_{:03}", stats.soul_animation_type);
            let rig = rigging(vfs, &base, std::slice::from_ref(&base))?;
            let file = vfs.find(&format!("{base}{MAANIM_EXT}")).map(|path| vfs.source(&path))?;

            (rig, file, None)
        }
        0 => return None,
        _ => (Arc::clone(rig?), own?, effect_length(vfs, SOUL_EFFECT, stats.soul_animation_type)),
    };

    Some(Motion {
        name: Some(SOUL_NAME.to_string()),
        slot: None,
        role: None,
        looping: Loop::Pose(Pose::Soul { effect }),
        rig,
        file: Some(file),
        combat: None,
    })
}

pub fn stance(stats: &Entity) -> [i32; 5] {
    [
        stats.spawn_animation_flag,
        stats.spawn_animation_type,
        stats.use_gudetama_soul,
        stats.soul_animation_type,
        stats.trait_zombie,
    ]
}

fn effect_length(vfs: &Vfs, family: &str, kind: i32) -> Option<i32> {
    (kind >= 0).then(|| {
        let name = format!("{family}_{kind:03}{MAANIM_EXT}");

        vfs.find(&name)
            .and_then(|path| vfs.read(&path).ok())
            .and_then(|bytes| Animation::scan_length(&bytes))
            .unwrap_or(0)
    })
}

pub(crate) fn standard_name(suffix: &str) -> Option<&'static str> {
    suffix.parse::<usize>().ok().and_then(standard).map(|(name, _, _)| name)
}

pub(crate) fn rigging(vfs: &Vfs, id: &str, bases: &[String]) -> Option<Arc<Rigging>> {
    let resolve = |ext: &str| -> Option<Source> {
        let names: Vec<String> = bases.iter().map(|base| format!("{}.{}", base, ext)).collect();

        vfs.find(names.as_slice()).map(|path| vfs.source(&path))
    };

    Some(Arc::new(Rigging {
        id: id.to_string(),
        png: resolve("png")?,
        cut: resolve("imgcut")?,
        model: resolve("mamodel")?,
    }))
}

fn owns(suffix: &str) -> bool {
    suffix.starts_with('_') || suffix.bytes().all(|byte| byte.is_ascii_digit())
}

fn maanim_files(vfs: &Vfs, bases: &[String]) -> Vec<(String, String, PathBuf)> {
    let mut found: Vec<(String, String, PathBuf)> = Vec::new();

    for base in bases {
        for name in vfs.glob(base) {
            let canonical = vfs.stripped(&name).unwrap_or_else(|| name.to_string());

            let Some(suffix) = canonical.strip_prefix(base.as_str()).and_then(|rest| rest.strip_suffix(MAANIM_EXT))
            else {
                continue;
            };

            if !owns(suffix) {
                continue;
            }

            if found.iter().any(|(existing, _, _)| existing == suffix) {
                continue;
            }

            if let Some(path) = vfs.find(&canonical) {
                found.push((suffix.to_string(), canonical, path));
            }
        }
    }

    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

pub(crate) fn maanims(vfs: &Vfs, bases: &[String]) -> Vec<(String, Source)> {
    maanim_files(vfs, bases).into_iter().map(|(suffix, _, path)| (suffix, vfs.source(&path))).collect()
}

pub struct RigFiles {
    pub key: String,
    pub sheet: String,
    pub cuts: String,
    pub model: String,
    pub anims: Vec<(String, String)>,
}

impl RigFiles {
    pub fn names(&self) -> Vec<String> {
        let rig = [self.sheet.as_str(), self.cuts.as_str(), self.model.as_str()];

        rig.into_iter().map(str::to_owned).chain(self.anims.iter().map(|(_, name)| name.clone())).collect()
    }
}

pub(crate) fn rig_files(vfs: &Vfs, id: &str, bases: &[String]) -> Option<RigFiles> {
    let resolve = |ext: &str| -> Option<String> {
        let names: Vec<String> = bases.iter().map(|base| format!("{}.{}", base, ext)).collect();
        let path = vfs.find(names.as_slice())?;

        path.file_name().and_then(OsStr::to_str).map(str::to_owned)
    };

    let anims = maanim_files(vfs, bases).into_iter().map(|(suffix, name, _)| (suffix, name)).collect();

    Some(RigFiles {
        key: id.to_string(),
        sheet: resolve("png")?,
        cuts: resolve("imgcut")?,
        model: resolve("mamodel")?,
        anims,
    })
}

pub fn loop_frame(animation: &Animation, frame: f32) -> f32 {
    seat(playback_frames(animation), frame)
}

pub fn restart(animation: &Animation) -> Option<i32> {
    cycle(animation).or_else(|| (animation.length() > 0).then(|| animation.declared_frames()))
}

fn seat(frames: i32, frame: f32) -> f32 {
    if frames > 0 { frame.rem_euclid(frames as f32) } else { frame }
}

pub fn playback_frames(animation: &Animation) -> i32 {
    cycle(animation).unwrap_or_else(|| animation.declared_frames())
}

pub fn cycle(animation: &Animation) -> Option<i32> {
    animation.loop_frames().filter(|frames| *frames <= LOOP_CEILING)
}

#[inline(always)]
pub fn multiply_mat3(matrix_a: &[f32; 9], matrix_b: &[f32; 9]) -> [f32; 9] {
    [
        matrix_a[0]*matrix_b[0] + matrix_a[3]*matrix_b[1] + matrix_a[6]*matrix_b[2],
        matrix_a[1]*matrix_b[0] + matrix_a[4]*matrix_b[1] + matrix_a[7]*matrix_b[2],
        matrix_a[2]*matrix_b[0] + matrix_a[5]*matrix_b[1] + matrix_a[8]*matrix_b[2],

        matrix_a[0]*matrix_b[3] + matrix_a[3]*matrix_b[4] + matrix_a[6]*matrix_b[5],
        matrix_a[1]*matrix_b[3] + matrix_a[4]*matrix_b[4] + matrix_a[7]*matrix_b[5],
        matrix_a[2]*matrix_b[3] + matrix_a[5]*matrix_b[4] + matrix_a[8]*matrix_b[5],

        matrix_a[0]*matrix_b[6] + matrix_a[3]*matrix_b[7] + matrix_a[6]*matrix_b[8],
        matrix_a[1]*matrix_b[6] + matrix_a[4]*matrix_b[7] + matrix_a[7]*matrix_b[8],
        matrix_a[2]*matrix_b[6] + matrix_a[5]*matrix_b[7] + matrix_a[8]*matrix_b[8],
    ]
}

pub fn name_prefix(set_name: &str, slug: &str) -> String {
    let mut parts = set_name.split('_');
    let mut clean = parts.next().unwrap_or("").to_owned();

    if let Some(form) = parts.next()
        && !clean.is_empty()
        && clean.chars().all(char::is_numeric)
    {
        let number = match form.chars().next() {
            Some('f') => 1,
            Some('c') => 2,
            Some('s') => 3,
            Some('u') => 4,
            _ => 0,
        };

        if number > 0 {
            clean = format!("{clean}-{number}");
        }
    }

    format!("{clean}.{slug}")
}



#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex::combat::{Entity, Faction};
    use codex::graphics::rig::{AnimModification, Animation, Keyframe, Pose};

    use crate::{Source, Vfs};

    use super::{cycle, own_soul, owns, playback_frames, restart, soul, Loop, Motion, Rigging, Role};

    fn curve(loop_count: i32, first: i32, last: i32) -> AnimModification {
        AnimModification {
            part: 0,
            kind: 0,
            loop_count,
            min_value: 0,
            max_value: 0,
            name: String::new(),
            keyframes: vec![
                Keyframe { frame: first, value: 0, ease: 0, ease_power: 0 },
                Keyframe { frame: last, value: 0, ease: 0, ease_power: 0 },
            ],
        }
    }

    fn animation(curves: Vec<AnimModification>) -> Animation {
        Animation { version: 1, modifications: curves }
    }

    #[test]
    fn a_play_once_curve_has_no_cycle_and_plays_its_declared_frames() {
        // An attack holds a loop_count of one, which is what makes the viewer show "???".
        let attack = animation(vec![curve(1, 0, 129)]);

        assert_eq!(cycle(&attack), None);
        assert_eq!(playback_frames(&attack), 130);
    }

    #[test]
    fn a_repeating_curve_keeps_its_cycle_as_a_frame_count() {
        // Spans 8 and 12 realign after 24 frames, and frame 24 is frame 0 again,
        // so the cycle is 24 frames rather than 25.
        let walk = animation(vec![curve(-1, 0, 8), curve(-1, 0, 12)]);

        assert_eq!(cycle(&walk), Some(24));
        assert_eq!(playback_frames(&walk), 24);
    }

    #[test]
    fn a_cycle_past_the_display_ceiling_falls_back_to_the_declared_frames() {
        // 9973 and 9967 are coprime, so the crate reports 99,400,891 frames. The
        // ceiling is ours: six digits is what the viewer's frame field can show.
        let absurd = animation(vec![curve(-1, 0, 9_973), curve(-1, 0, 9_967)]);

        assert_eq!(absurd.loop_frames(), Some(99_400_891));
        assert_eq!(cycle(&absurd), None);
        assert_eq!(playback_frames(&absurd), absurd.declared_frames());
    }

    // 872_f00 in the shipped corpus: 103 modifications, 99 of them replaying forever over
    // spans of 15, 18, 45, 60 and 200, and four that play exactly once. The play-once four
    // are why loop_frames gives up, and the forever ninety-nine are why frame 200 is not a
    // seam - so restarting there snaps every one of them mid-motion.
    #[test]
    fn a_timeline_that_never_ends_has_no_restart_to_snap_to() {
        let walk = animation(vec![curve(-1, 0, 60), curve(-1, 0, 15), curve(1, 0, 200)]);

        assert_eq!(walk.length(), -1);
        assert_eq!(walk.loop_frames(), None);
        assert_eq!(walk.declared_frames(), 200);

        assert_eq!(restart(&walk), None);
        assert_eq!(Loop::Auto.seat(&walk, 260.0), 260.0);

        // The old bound wrapped at the declared count, which is what snapped.
        assert_eq!(playback_frames(&walk), 200);
    }

    #[test]
    fn a_timeline_that_ends_still_restarts_at_its_declared_length() {
        let attack = animation(vec![curve(1, 0, 89)]);

        assert_eq!(attack.length(), 90);
        assert_eq!(restart(&attack), Some(90));
        assert_eq!(Loop::Auto.seat(&attack, 90.0), 0.0);
    }

    #[test]
    fn a_timeline_with_a_cycle_restarts_on_the_cycle() {
        let walk = animation(vec![curve(-1, 0, 8), curve(-1, 0, 12)]);

        assert_eq!(restart(&walk), Some(24));
        assert_eq!(Loop::Auto.seat(&walk, 25.0), 1.0);
    }

    #[test]
    fn a_looping_animation_no_longer_replays_its_first_frame() {
        let walk = animation(vec![curve(-1, 0, 16)]);

        // 0..=15 are distinct; frame 16 renders as frame 0, so it must wrap there.
        assert_eq!(super::loop_frame(&walk, 15.0), 15.0);
        assert_eq!(super::loop_frame(&walk, 16.0), 0.0);
    }

    #[test]
    fn the_authored_range_and_the_playback_bound_agree_without_a_cycle() {
        // The exporter sizes its Frames row from declared_frames - 1 and the viewer
        // sizes its slider from playback_frames - 1. Those must land on the same
        // number, or the two controls disagree about the same animation.
        let attack = animation(vec![curve(1, 0, 15)]);
        assert_eq!(attack.declared_frames() - 1, playback_frames(&attack) - 1);

        // A held pose is one frame, so index zero from both directions.
        let knockback = animation(vec![curve(-1, 0, 0)]);
        assert_eq!(knockback.declared_frames() - 1, playback_frames(&knockback) - 1);

        // A looping walk ends one frame before its furthest keyframe, since that
        // frame renders as frame zero again.
        let walk = animation(vec![curve(-1, 0, 16)]);
        assert_eq!(walk.declared_frames() - 1, 15);
        assert_eq!(playback_frames(&walk) - 1, 15);
    }

    #[test]
    fn a_soul_only_shows_when_it_is_the_units_own() {
        // Soul type 0 is the shared default soul and -1 is none at all, so neither earns a
        // button. A unit with the custom soul flag shows its own file on its own rig.
        let vfs = Vfs::with_priority(&[]);
        let rig = Arc::new(Rigging { id: "rig".to_owned(), png: Source::disk("a.png"), cut: Source::disk("a.imgcut"), model: Source::disk("a.mamodel") });
        let plain = Entity { soul_animation_type: 0, ..Entity::default() };
        let none = Entity { soul_animation_type: -1, ..Entity::default() };
        let custom = Entity { soul_animation_type: -1, use_gudetama_soul: 1, ..Entity::default() };

        assert!(soul(&vfs, &plain, Some(&rig), None).is_none());
        assert!(soul(&vfs, &none, Some(&rig), None).is_none());
        assert!(soul(&vfs, &custom, Some(&rig), None).is_none(), "the flag without the file has nothing to show");

        let shown = soul(&vfs, &custom, Some(&rig), Some(Source::disk("a_soul.maanim")));

        assert_eq!(shown.as_ref().map(Motion::label).as_deref(), Some("Soul"));
        assert_eq!(shown.map(|motion| motion.looping), Some(Loop::Pose(Pose::Soul { effect: None })));
        assert!(own_soul("_soul", Some(&custom)));
        assert!(!own_soul("_soul", Some(&plain)));
    }

    #[test]
    fn a_longer_name_sharing_the_prefix_is_not_ours() {
        // 008_charaawa.maanim starts with 008_c but belongs to a different unit.
        assert!(!owns("haraawa"));
        assert!(!owns("haraawa_zombie00"));
    }

    #[test]
    fn numbered_and_underscored_suffixes_are_ours() {
        assert!(owns("00"));
        assert!(owns("14"));
        assert!(owns("_zombie02"));
        assert!(owns(""));
    }

    #[test]
    fn a_knockback_plays_exactly_24_frames_whatever_its_file_says() {
        // A forever track beside a play-once one used to leave this unbounded, and a
        // longer file used to run past the 24 frames a knockback ever shows.
        let mixed = animation(vec![curve(-1, 0, 6), curve(1, 0, 60)]);

        let knockback = Loop::Pose(Pose::Knockback);

        assert_eq!(knockback.bound(&mixed), Some(23));
        assert_eq!(knockback.frames(&mixed), 24);
        assert_eq!(knockback.seat(&mixed, 23.0), 23.0);
        assert_eq!(knockback.seat(&mixed, 24.0), 0.0);
        assert_eq!(knockback.exported(&mixed, 30.0), 6.0);
        assert!(!knockback.native(Some(&mixed)));
    }

    #[test]
    fn an_endless_track_in_a_cat_attack_stops_at_the_furthest_key() {
        // The game ends the attack where its counter wraps, so the forever track only gets
        // as many laps as fit before frame 30, and never runs on past it.
        let attack = animation(vec![curve(-1, 0, 10), curve(1, 0, 30)]);
        let looping = Role::Attack.looping(Faction::Cat);

        assert_eq!(looping.bound(&attack), Some(29));
        assert_eq!(Role::Attack.looping(Faction::Enemy).bound(&attack), Some(0));
    }

    #[test]
    fn an_attack_is_drawn_from_frame_one() {
        // The frame it starts on is advanced before the first draw, so frame 0 never shows,
        // and a wrap past the end lands back on frame 1. A scrub to 0 still shows frame 0.
        let attack = animation(vec![curve(1, 0, 30)]);
        let looping = Role::Attack.looping(Faction::Cat);

        assert_eq!(looping.first(&attack), 1);
        assert_eq!(looping.bound(&attack), Some(30));
        assert_eq!(looping.period(&attack), 30);
        assert_eq!(looping.seat(&attack, 31.0), 1.0);
        assert_eq!(looping.seat(&attack, 0.0), 0.0);
    }

    #[test]
    fn a_walk_never_wraps_unless_it_truly_repeats() {
        // The walk counter only ever counts up, so the play-once track holds and the
        // viewer has nowhere exact to restart.
        let held = animation(vec![curve(-1, 0, 10), curve(1, 0, 30)]);
        let walk = Role::Walk.looping(Faction::Cat);

        assert_eq!(walk.bound(&held), None);
        assert_eq!(walk.seat(&held, 45.0), 45.0);

        let cycling = animation(vec![curve(-1, 0, 8), curve(-1, 0, 12)]);

        assert_eq!(walk.bound(&cycling), Some(23));
        assert_eq!(walk.seat(&cycling, 25.0), 1.0);
    }
}
