use serde::Serialize;

use crate::combat::Faction;
use crate::common::columns::{self, Column};
use crate::common::stream::{declared_rows, Cursor, Reader};
use crate::graphics::tools::math;

use super::RigError;

const POWERED_VERSION: i32 = 1;

const ENDLESS: i32 = -1;

pub const KNOCKBACK_LENGTH: i32 = 24;

const BURROW_LENGTH: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pose {
    Walk,
    Idle,
    Attack(Faction),
    Knockback,
    Burrow,
    Dig,
    Surface,
    Entry { effect: Option<i32> },
    Soul { effect: Option<i32> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Counter {
    Rising,
    Wrapping { from: i32, length: u32 },
    Faulting,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Keyframe {
    pub frame: i32,
    pub value: i32,
    pub ease: i32,
    pub ease_power: i32,
}

impl Keyframe {
    pub const COLUMNS: &'static [Column<Self>] = columns::columns! {
        frame      : 0;
        value      : 1;
        ease       : 2;
        ease_power : 3;
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AnimModification {
    pub part: i32,
    pub kind: i32,
    pub loop_count: i32,
    pub min_value: i32,
    pub max_value: i32,
    pub name: String,
    pub keyframes: Vec<Keyframe>,
}

impl AnimModification {
    pub const COLUMNS: &'static [Column<Self>] = columns::columns! {
        part       : 0;
        kind       : 1;
        loop_count : 2;
        min_value  : 3;
        max_value  : 4;
    };
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Animation {
    pub version: i32,
    pub modifications: Vec<AnimModification>,
}

impl Animation {
    pub fn parse(bytes: impl AsRef<[u8]>) -> Result<Self, RigError> {
        Self::parse_inner(bytes.as_ref())
    }

    fn parse_inner(bytes: &[u8]) -> Result<Self, RigError> {
        let content = String::from_utf8_lossy(bytes);
        let mut reader = Reader::new(&content);

        if reader.line().is_none() { return Err(RigError::EmptyFile); }

        reader.row();
        let version = reader.value(0);

        reader.row();
        let count = declared_rows(reader.value(0)).ok_or(RigError::CountTooLarge)?;

        let mut modifications = Vec::with_capacity(count);

        for _ in 0..count {
            reader.row();

            let mut modification = AnimModification::default();
            let trailing = columns::apply(&reader.scalars(), AnimModification::COLUMNS, &mut modification);
            modification.name = reader.cells().get(trailing).map(|text| text.trim().to_string()).unwrap_or_default();

            reader.row();
            let keys = declared_rows(reader.value(0)).ok_or(RigError::CountTooLarge)?;

            modification.keyframes = Vec::with_capacity(keys);

            for _ in 0..keys {
                reader.row();

                let mut keyframe = Keyframe::default();
                columns::apply(&reader.scalars(), Keyframe::COLUMNS, &mut keyframe);

                if version < POWERED_VERSION { keyframe.ease_power = 0; }

                modification.keyframes.push(keyframe);
            }

            modifications.push(modification);
        }

        Ok(Self { version, modifications })
    }

    pub fn length(&self) -> i32 {
        let mut longest = 0;

        for modification in &self.modifications {
            if modification.loop_count == -1 { return -1; }

            let (Some(first), Some(last)) = (modification.keyframes.first(), modification.keyframes.last()) else {
                continue;
            };

            let span = last.frame.wrapping_sub(first.frame);
            let played = modification.loop_count
                .wrapping_sub(1)
                .wrapping_mul(span)
                .wrapping_add(last.frame);

            if played >= longest { longest = played.wrapping_add(1); }
        }

        longest
    }

    pub fn counter(&self, pose: Pose) -> Counter {
        match pose {
            Pose::Walk | Pose::Dig => Counter::Rising,
            Pose::Idle if self.last_frame() == 0 => Counter::Wrapping { from: 0, length: 1 },
            Pose::Idle => Counter::Rising,
            Pose::Attack(faction) => match self.attack_divisor(faction) {
                0 => Counter::Faulting,
                divisor => Counter::Wrapping { from: 1, length: divisor.unsigned_abs() },
            },
            Pose::Knockback => Counter::Wrapping { from: 0, length: KNOCKBACK_LENGTH.unsigned_abs() },
            Pose::Burrow | Pose::Surface => Counter::Wrapping { from: 0, length: BURROW_LENGTH },
            Pose::Entry { effect } => Counter::Wrapping { from: 1, length: self.stretched(effect).unsigned_abs() },
            Pose::Soul { effect } => Counter::Wrapping { from: 0, length: self.stretched(effect).unsigned_abs() + 1 },
        }
    }

    pub fn attack_length(&self, faction: Faction) -> i32 {
        self.attack_divisor(faction).saturating_abs()
    }

    fn attack_divisor(&self, faction: Faction) -> i32 {
        match (faction, self.length()) {
            (Faction::Cat, ENDLESS) => self.last_frame(),
            (_, length) => length,
        }
    }

    fn stretched(&self, effect: Option<i32>) -> i32 {
        let own = match self.length() {
            length if length >= 2 => length,
            _ => 1,
        };

        effect.filter(|length| *length > own).unwrap_or(own)
    }

    pub fn period(&self) -> Option<i32> {
        let mut period: i32 = 1;

        for modification in &self.modifications {
            let (Some(first), Some(last)) = (modification.keyframes.first(), modification.keyframes.last()) else {
                continue;
            };

            if first.frame > 0 { return None; }

            let count = modification.loop_count;
            let span = i64::from(last.frame) - i64::from(first.frame);
            let settled = match count {
                count if count > 0 => i64::from(first.frame) + i64::from(count) * span,
                _ => i64::from(last.frame),
            };

            let repeat = match span {
                0 => 1,
                span if span < 0 && (count == ENDLESS || count > 0) => i32::try_from(-span).ok()?,
                span if span < 0 => 1,
                span if count == ENDLESS || settled > i64::from(i32::MAX) => i32::try_from(span).ok()?,
                _ if settled <= 0 => 1,
                _ => return None,
            };

            period = (period / math::gcd(period, repeat)).checked_mul(repeat)?;
        }

        Some(period)
    }

    pub fn declared_frames(&self) -> i32 {
        match self.length() {
            length if length > 0 => length,
            _ => self.last_frame().max(1),
        }
    }

    pub fn last_frame(&self) -> i32 {
        self.modifications.iter()
            .filter_map(|modification| modification.keyframes.last())
            .fold(0, |furthest, keyframe| furthest.max(keyframe.frame))
    }

    pub fn loop_frames(&self) -> Option<i32> {
        let last = self.last_frame();
        let mut cycle: i32 = 1;
        let mut repeating = false;

        for modification in &self.modifications {
            if modification.loop_count == 1 { return None; }

            let (Some(first), Some(final_key)) = (modification.keyframes.first(), modification.keyframes.last()) else {
                continue;
            };

            let span = final_key.frame - first.frame;
            if span <= 0 { continue; }

            cycle = (cycle / math::gcd(cycle, span)).checked_mul(span)?;
            repeating = true;
        }

        if !repeating { return Some(last + 1); }

        (cycle >= last).then_some(cycle)
    }

    pub fn playback_frames(&self) -> i32 {
        self.loop_frames().unwrap_or_else(|| self.declared_frames())
    }

    pub fn scan_length(bytes: impl AsRef<[u8]>) -> Option<i32> {
        Self::scan_length_inner(bytes.as_ref())
    }

    pub fn scan_attack(bytes: impl AsRef<[u8]>, faction: Faction) -> Option<i32> {
        Self::scan_attack_inner(bytes.as_ref(), faction)
    }

    fn scan_attack_inner(bytes: &[u8], faction: Faction) -> Option<i32> {
        match Self::scan_length_inner(bytes)? {
            ENDLESS => Self::parse_inner(bytes).ok().map(|anim| anim.attack_length(faction)),
            length => Some(length.saturating_abs()),
        }
    }

    fn scan_length_inner(bytes: &[u8]) -> Option<i32> {
        let mut cursor = Cursor::new(bytes);

        cursor.line()?;

        cursor.skip(2);
        let count = declared_rows(cursor.value(0))?;

        let mut longest = 0;

        for _ in 0..count {
            cursor.row();
            let loop_count = cursor.value(2);

            if loop_count == -1 { return Some(-1); }

            cursor.row();
            let keys = declared_rows(cursor.value(0))?;

            if keys == 0 { continue; }

            cursor.row();
            let first = cursor.value(0);

            cursor.skip(keys - 1);
            let last = cursor.value(0);

            let played = loop_count
                .wrapping_sub(1)
                .wrapping_mul(last.wrapping_sub(first))
                .wrapping_add(last);

            if played >= longest { longest = played.wrapping_add(1); }
        }

        Some(longest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(version: i32, tail: &[&str]) -> String {
        let mut lines = vec!["[modelanim:animation]".to_string(), version.to_string()];
        lines.extend(tail.iter().map(|line| (*line).to_string()));

        lines.join("\n")
    }

    #[test]
    fn a_version_below_one_carries_no_easing_exponent() {
        let tail = ["1", "0,4,1,0,0,move", "1", "0,50,3,7"];

        assert_eq!(
            Animation::parse(file(0, &tail)).map(|anim| anim.modifications[0].keyframes[0].ease_power),
            Ok(0),
        );
        assert_eq!(
            Animation::parse(file(1, &tail)).map(|anim| anim.modifications[0].keyframes[0].ease_power),
            Ok(7),
        );
    }

    #[test]
    fn a_blank_line_is_a_row_like_any_other() {
        let anim = Animation::parse(file(1, &["1", "", "1", "5,50,0,0"])).expect("a timeline parses");

        assert_eq!(anim.modifications[0].part, 0);
        assert_eq!(anim.modifications[0].loop_count, 0);
        assert_eq!(anim.modifications[0].keyframes.len(), 1);
    }

    #[test]
    fn a_file_that_runs_out_repeats_the_last_row_it_read() {
        let truncated = file(1, &["1", "0,4,1,0,0,move", "3", "5,50,0,0"]);
        let anim = Animation::parse(&truncated).expect("a timeline parses");

        let keyframes = &anim.modifications[0].keyframes;

        assert_eq!(keyframes.len(), 3);
        assert_eq!(keyframes[1], keyframes[0]);
        assert_eq!(keyframes[2], keyframes[0]);
        assert_eq!(Animation::scan_length(&truncated), Some(anim.length()));
    }

    #[test]
    fn a_count_the_engine_aborts_on_is_refused() {
        assert_eq!(Animation::parse(file(1, &["-1"])), Err(RigError::CountTooLarge));
        assert_eq!(Animation::parse(file(1, &["1", "0,4,1,0,0", "-2"])), Err(RigError::CountTooLarge));
        assert_eq!(Animation::parse(""), Err(RigError::EmptyFile));
    }

    #[test]
    fn a_declared_modification_survives_holding_no_keyframes() {
        let anim = Animation::parse(file(1, &["2", "0,4,1,0,0", "0", "1,4,-1,0,0", "0"]))
            .expect("a timeline parses");

        assert_eq!(anim.modifications.len(), 2);
        assert_eq!(anim.length(), -1);
    }

    fn modification(loop_count: i32, frames: &[i32]) -> AnimModification {
        AnimModification {
            loop_count,
            keyframes: frames.iter().map(|&frame| Keyframe { frame, ..Keyframe::default() }).collect(),
            ..AnimModification::default()
        }
    }

    #[test]
    fn keyframe_columns_map_one_field_each() {
        columns::assert_one_field_per_column(Keyframe::COLUMNS);
    }

    #[test]
    fn modification_columns_map_one_field_each() {
        columns::assert_one_field_per_column(AnimModification::COLUMNS);
    }

    #[test]
    fn length_counts_every_replay() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(3, &[2, 12]), modification(1, &[0, 5])],
        };

        assert_eq!(animation.length(), 33);
    }

    #[test]
    fn an_endless_timeline_has_no_length() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(4, &[0, 30]), modification(-1, &[0, 8])],
        };

        assert_eq!(animation.length(), -1);
    }

    #[test]
    fn period_is_the_common_multiple_of_endless_spans() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 8]), modification(-1, &[0, 12])],
        };

        assert_eq!(animation.period(), Some(24));
    }

    #[test]
    fn declared_frames_ignores_the_realignment_interval() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 8]), modification(-1, &[0, 140])],
        };

        assert_eq!(animation.playback_frames(), 280);
        assert_eq!(animation.declared_frames(), 140);
    }

    #[test]
    fn declared_frames_counts_a_timeline_that_ends() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(1, &[0, 152])],
        };

        assert_eq!(animation.declared_frames(), 153);
    }

    #[test]
    fn loop_frames_counts_the_realignment_interval_without_padding() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 8]), modification(-1, &[0, 12])],
        };

        assert_eq!(animation.loop_frames(), Some(24));
    }

    #[test]
    fn loop_frames_refuses_an_interval_that_truncates_the_content() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 50]), modification(-1, &[84, 84])],
        };

        assert_eq!(animation.loop_frames(), None);
        assert_eq!(animation.playback_frames(), animation.declared_frames());
        assert_eq!(animation.declared_frames(), 84);
    }

    #[test]
    fn loop_frames_reports_an_interval_no_player_would_use() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 9_973]), modification(-1, &[0, 9_967])],
        };

        assert_eq!(animation.loop_frames(), Some(99_400_891));
    }

    #[test]
    fn a_play_once_modification_has_no_cycle() {
        let animation = Animation { version: 1, modifications: vec![modification(1, &[0, 129])] };

        assert_eq!(animation.loop_frames(), None);
        assert_eq!(animation.playback_frames(), 130);
    }

    #[test]
    fn a_held_pose_occupies_the_frames_it_declares() {
        let animation = Animation { version: 1, modifications: vec![modification(-1, &[7, 7])] };

        assert_eq!(animation.loop_frames(), Some(8));
    }

    #[test]
    fn last_frame_is_not_the_declared_count_less_one() {
        let animation = Animation { version: 1, modifications: vec![modification(3, &[2, 12])] };

        assert_eq!(animation.last_frame(), 12);
        assert_eq!(animation.declared_frames(), 33);
    }

    #[test]
    fn period_rejects_a_timeline_that_stops() {
        let animation = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 8]), modification(1, &[0, 12])],
        };

        assert_eq!(animation.period(), None);
    }

    #[test]
    fn period_rejects_a_track_that_only_counts_a_few_laps() {
        // Loop 3 holds its last key after three laps, so frame 30 onwards never matches frame 0.
        let laps = Animation { version: 1, modifications: vec![modification(3, &[0, 10])] };

        assert_eq!(laps.period(), None);

        // Past the counter's own range the laps never run out.
        let unending = Animation { version: 1, modifications: vec![modification(i32::MAX, &[0, 10])] };

        assert_eq!(unending.period(), Some(10));
    }

    #[test]
    fn period_rejects_a_track_that_starts_late() {
        // The part sits at rest until frame 5 and never again, so frame 0 is never repeated.
        let late = Animation { version: 1, modifications: vec![modification(-1, &[5, 15])] };

        assert_eq!(late.period(), None);
    }

    #[test]
    fn a_track_that_starts_before_frame_zero_still_repeats() {
        // The forever track is already mid-lap at frame 0 and laps every 12; the play-once
        // track finished at frame -5, so it only ever shows its last key.
        let early = Animation {
            version: 1,
            modifications: vec![modification(-1, &[-6, 6]), modification(1, &[-20, -5])],
        };

        assert_eq!(early.period(), Some(12));
    }

    #[test]
    fn a_still_timeline_repeats_every_frame() {
        let still = Animation { version: 1, modifications: vec![modification(-1, &[0, 0]), modification(1, &[-4, -4])] };

        assert_eq!(still.period(), Some(1));
    }

    #[test]
    fn an_endless_track_ends_a_cat_attack_at_the_furthest_key() {
        let attack = Animation {
            version: 1,
            modifications: vec![modification(-1, &[0, 10]), modification(1, &[0, 30])],
        };

        assert_eq!(attack.counter(Pose::Attack(Faction::Cat)), Counter::Wrapping { from: 1, length: 30 });
        assert_eq!(attack.counter(Pose::Attack(Faction::Enemy)), Counter::Wrapping { from: 1, length: 1 });
        assert_eq!(attack.counter(Pose::Walk), Counter::Rising);
    }

    #[test]
    fn a_cat_attack_with_nothing_past_frame_zero_divides_by_zero() {
        let stuck = Animation { version: 1, modifications: vec![modification(-1, &[0, 0])] };

        assert_eq!(stuck.counter(Pose::Attack(Faction::Cat)), Counter::Faulting);
        assert_eq!(stuck.counter(Pose::Attack(Faction::Enemy)), Counter::Wrapping { from: 1, length: 1 });
        assert_eq!(stuck.counter(Pose::Idle), Counter::Wrapping { from: 0, length: 1 });
    }

    #[test]
    fn a_plain_attack_wraps_at_its_length() {
        let attack = Animation { version: 1, modifications: vec![modification(3, &[2, 12])] };

        assert_eq!(attack.counter(Pose::Attack(Faction::Cat)), Counter::Wrapping { from: 1, length: 33 });
        assert_eq!(attack.counter(Pose::Attack(Faction::Enemy)), Counter::Wrapping { from: 1, length: 33 });
        assert_eq!(attack.counter(Pose::Knockback), Counter::Wrapping { from: 0, length: 24 });
    }

    #[test]
    fn an_entry_runs_for_its_own_length_or_its_effect_whichever_is_longer() {
        // Anything shorter than two frames counts as one, and so does an endless entry.
        let entry = Animation { version: 1, modifications: vec![modification(1, &[0, 19])] };
        let endless = Animation { version: 1, modifications: vec![modification(-1, &[0, 19])] };

        assert_eq!(entry.counter(Pose::Entry { effect: None }), Counter::Wrapping { from: 1, length: 20 });
        assert_eq!(entry.counter(Pose::Entry { effect: Some(45) }), Counter::Wrapping { from: 1, length: 45 });
        assert_eq!(endless.counter(Pose::Entry { effect: None }), Counter::Wrapping { from: 1, length: 1 });
    }

    #[test]
    fn a_soul_shows_one_frame_past_its_length() {
        // Dying counts up while the frame is under the limit, so the limit itself is drawn too.
        let soul = Animation { version: 1, modifications: vec![modification(1, &[0, 29])] };

        assert_eq!(soul.counter(Pose::Soul { effect: None }), Counter::Wrapping { from: 0, length: 31 });
        assert_eq!(soul.counter(Pose::Soul { effect: Some(10) }), Counter::Wrapping { from: 0, length: 31 });
    }

    #[test]
    fn a_zombie_burrows_and_surfaces_over_thirty_frames_and_digs_without_end() {
        let any = Animation { version: 1, modifications: vec![modification(-1, &[0, 7])] };

        assert_eq!(any.counter(Pose::Burrow), Counter::Wrapping { from: 0, length: 30 });
        assert_eq!(any.counter(Pose::Surface), Counter::Wrapping { from: 0, length: 30 });
        assert_eq!(any.counter(Pose::Dig), Counter::Rising);
    }

    #[test]
    fn an_attack_scan_reads_an_endless_cat_attack_to_its_furthest_key() {
        let endless = file(1, &["2", "0,4,-1,0,0", "2", "0,0,0,0", "10,5,0,0", "1,4,1,0,0", "2", "0,0,0,0", "30,5,0,0"]);

        assert_eq!(Animation::scan_attack(&endless, Faction::Cat), Some(30));
        assert_eq!(Animation::scan_attack(&endless, Faction::Enemy), Some(1));
        assert_eq!(Animation::scan_attack(file(1, &["1", "0,4,1,0,0", "2", "0,0,0,0", "30,5,0,0"]), Faction::Cat), Some(31));
    }
}
