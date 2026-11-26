use super::{Entity, Faction};

pub(super) const RANGE_STEPS: i32 = 4;
pub(super) const WAVE_STRIDE: i32 = 800;
pub(super) const WAVE_LEAD: i32 = 270;
const STEPS_PER_PIXEL: f32 = 10.0;
const KNOCKBACK_STEP: i32 = 10;

pub const KNOCKBACK_FRAMES: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub near: f32,
    pub far: f32,
}

impl Span {
    fn ordered(from: f32, to: f32) -> Self {
        Self { near: from.min(to), far: from.max(to) }
    }
}

impl Entity {
    pub fn strike_frames(&self) -> Vec<i32> {
        [self.time_until_attack_1, self.time_until_attack_2, self.time_until_attack_3]
            .into_iter()
            .take_while(|frame| *frame != 0)
            .collect()
    }

    pub(super) fn strike_band(&self, attack: usize) -> (i32, i32) {
        let first = (self.long_distance_1_anchor, self.long_distance_1_span);

        let (flag, own) = match attack {
            1 => (self.long_distance_2_flag, (self.long_distance_2_anchor, self.long_distance_2_span)),
            2 => (self.long_distance_3_flag, (self.long_distance_3_anchor, self.long_distance_3_span)),
            _ => (0, first),
        };

        let (anchor, span) = if flag != 0 { own } else { first };

        if anchor == 0 {
            return (0, self.standing_range);
        }

        let end = anchor.wrapping_add(span);

        (anchor.min(end), anchor.max(end))
    }

    pub fn strike_reach(&self, attack: usize) -> Span {
        let (near, far) = self.strike_band(attack);

        Span { near: pixels(near), far: pixels(far) }
    }

    pub fn standing_reach(&self) -> f32 {
        pixels(self.standing_range)
    }

    pub fn hurtbox(&self) -> Span {
        Span::ordered(self.hitbox_position as f32 / STEPS_PER_PIXEL, pixels(self.hitbox_width))
    }

    pub(crate) fn wave_front(&self) -> i32 {
        let span = WAVE_STRIDE.saturating_mul(self.wave_level.saturating_add(1));

        match self.faction {
            Faction::Cat => span.saturating_sub(WAVE_LEAD),
            Faction::Enemy => span.saturating_add(WAVE_LEAD),
        }
    }

    pub fn wave_reach(&self) -> f32 {
        range_of(self.wave_front())
    }
}

pub fn range_of(position: i32) -> f32 {
    position as f32 / RANGE_STEPS as f32
}

pub fn knockback_push(fruit: i32, raise: i32, resist: i32) -> i32 {
    (1..KNOCKBACK_FRAMES)
        .map(|frame| {
            let phase = (KNOCKBACK_FRAMES - frame) % KNOCKBACK_FRAMES * KNOCKBACK_STEP;
            let fruited = fruit * phase / 1000 + phase;
            let raised = (100 + raise) * fruited / 100;

            (100 - resist) * raised / 100
        })
        .sum()
}

pub fn knockback_reach(fruit: i32, raise: i32, resist: i32) -> f32 {
    range_of(knockback_push(fruit, raise, resist))
}

fn pixels(range: i32) -> f32 {
    range as f32 * RANGE_STEPS as f32 / STEPS_PER_PIXEL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_attacker_reaches_from_itself_out_to_its_range() {
        let stats = Entity { standing_range: 140, time_until_attack_1: 8, ..Entity::default() };

        assert_eq!(stats.strike_reach(0), Span { near: 0.0, far: 56.0 });
        assert_eq!(stats.strike_frames(), vec![8]);
    }

    #[test]
    fn a_later_hit_borrows_the_first_band_unless_its_own_flag_is_set() {
        let mut stats = Entity {
            long_distance_1_anchor: 100,
            long_distance_1_span: 200,
            long_distance_2_anchor: 500,
            long_distance_2_span: -100,
            ..Entity::default()
        };

        assert_eq!(stats.strike_reach(1), Span { near: 40.0, far: 120.0 });

        stats.long_distance_2_flag = 1;

        // A negative span runs back toward the unit, so the ends swap.
        assert_eq!(stats.strike_reach(1), Span { near: 160.0, far: 200.0 });
    }

    #[test]
    fn a_flagged_hit_with_no_anchor_falls_back_to_standing_range() {
        let stats = Entity {
            standing_range: 50,
            long_distance_1_anchor: 100,
            long_distance_1_span: 200,
            long_distance_3_flag: 1,
            ..Entity::default()
        };

        assert_eq!(stats.strike_reach(2), Span { near: 0.0, far: 20.0 });
    }

    // The engine steps a wave 800 positions at a time, and a range unit is four positions,
    // which is where the community's .5 comes from.
    #[test]
    fn a_wave_reaches_the_community_distances() {
        let cat = |wave_level| Entity { wave_level, ..Entity::default() };
        let enemy = |wave_level| Entity { faction: Faction::Enemy, wave_level, ..Entity::default() };

        assert_eq!(cat(1).wave_front(), 1330);
        assert_eq!(cat(1).wave_reach(), 332.5);
        assert_eq!(cat(3).wave_reach(), 732.5);
        assert_eq!(enemy(1).wave_reach(), 467.5);
        assert_eq!(enemy(2).wave_reach(), 667.5);
    }

    #[test]
    fn knockback_rounds_every_frame_of_its_push() {
        assert_eq!(knockback_reach(0, 0, 0), 165.0);
        assert_eq!(knockback_reach(300, 0, 0), 214.5);
        assert_eq!(knockback_reach(0, 0, 50), 82.5);
        assert_eq!(knockback_reach(300, 0, 50), 106.5, "halving 13 per step drops a half on every odd frame");
        assert_eq!(knockback_reach(0, 20, 0), 198.0);
    }

    #[test]
    fn hits_stop_counting_at_the_first_empty_foreswing() {
        let stats = Entity { time_until_attack_1: 5, time_until_attack_3: 30, ..Entity::default() };

        assert_eq!(stats.strike_frames(), vec![5]);
    }
}
