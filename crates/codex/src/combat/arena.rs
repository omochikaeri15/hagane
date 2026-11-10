use super::{Entity, Faction};

const RANGE_STEPS: f32 = 4.0;
const STEPS_PER_PIXEL: f32 = 10.0;

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

impl Faction {
    pub fn bound_row(self) -> usize {
        match self {
            Faction::Cat => 1,
            Faction::Enemy => 0,
        }
    }

    pub fn bound_depth(self, reach: f32) -> f32 {
        match self {
            Faction::Cat => reach * 2.0,
            Faction::Enemy => reach,
        }
    }
}

impl Entity {
    pub fn strike_frames(&self) -> Vec<i32> {
        [self.time_until_attack_1, self.time_until_attack_2, self.time_until_attack_3]
            .into_iter()
            .take_while(|frame| *frame != 0)
            .collect()
    }

    pub fn strike_reach(&self, attack: usize) -> Span {
        let first = (self.long_distance_1_anchor, self.long_distance_1_span);

        let (flag, own) = match attack {
            1 => (self.long_distance_2_flag, (self.long_distance_2_anchor, self.long_distance_2_span)),
            2 => (self.long_distance_3_flag, (self.long_distance_3_anchor, self.long_distance_3_span)),
            _ => (0, first),
        };

        let (anchor, span) = if flag != 0 { own } else { first };

        if anchor == 0 {
            return Span { near: 0.0, far: pixels(self.standing_range) };
        }

        Span::ordered(pixels(anchor), pixels(anchor.wrapping_add(span)))
    }

    pub fn standing_reach(&self) -> f32 {
        pixels(self.standing_range)
    }

    pub fn hurtbox(&self) -> Span {
        Span::ordered(self.hitbox_position as f32 / STEPS_PER_PIXEL, pixels(self.hitbox_width))
    }
}

fn pixels(range: i32) -> f32 {
    range as f32 * RANGE_STEPS / STEPS_PER_PIXEL
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

    #[test]
    fn hits_stop_counting_at_the_first_empty_foreswing() {
        let stats = Entity { time_until_attack_1: 5, time_until_attack_3: 30, ..Entity::default() };

        assert_eq!(stats.strike_frames(), vec![5]);
    }
}
