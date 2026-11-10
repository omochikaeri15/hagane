use std::sync::Arc;

use codex::combat::{Entity, Faction, Span};
use codex::graphics::animate;
use codex::graphics::rig::{Animation, Rig};

use crate::domains::settings::{CombatLook, HitPositions};

pub type Color = [f32; 4];
pub type Point = (f32, f32);

pub const STRIKE_COLORS: [Color; 3] = [[1.0, 0.12, 0.12, 1.0], [1.0, 0.55, 0.0, 1.0], [1.0, 0.95, 0.1, 1.0]];
pub const HURTBOX_COLOR: Color = [0.1, 1.0, 0.9, 1.0];
pub const ORIGIN_COLOR: Color = [0.72, 0.3, 1.0, 1.0];
pub const BOUND_COLOR: Color = [1.0, 0.35, 0.75, 1.0];
pub const RANGE_COLOR: Color = [0.58, 0.32, 0.12, 1.0];
pub const BASE_COLOR: Color = [0.82, 0.82, 0.82, 1.0];
pub const SWEEP_COLOR: Color = [0.2, 0.9, 0.3, 1.0];
pub const BLAST_COLORS: [Color; 3] = [[0.05, 0.5, 0.15, 1.0], [0.2, 0.75, 0.3, 1.0], [0.55, 1.0, 0.6, 1.0]];
pub const CANNON_COLOR: Color = [0.1, 0.2, 0.8, 1.0];

pub const OUTLINE_WIDTH: f32 = 2.0;
pub const LINE_WIDTH: f32 = 2.0;
pub const FRAMES_PER_SECOND: f32 = 30.0;
pub const LINE_RISE: f32 = 200.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    faction: Faction,
    hurtbox: Span,
    range: f32,
    strikes: Vec<(i32, Span)>,
}

impl Profile {
    pub fn of(stats: &Entity) -> Self {
        let strikes = stats
            .strike_frames()
            .into_iter()
            .enumerate()
            .map(|(attack, frame)| (frame, stats.strike_reach(attack)))
            .collect();

        Self { faction: stats.faction, hurtbox: stats.hurtbox(), range: stats.standing_reach(), strikes }
    }

    pub fn measured(faction: Faction, hurtbox: Span, range: f32, strikes: Vec<(i32, Span)>) -> Self {
        Self { faction, hurtbox, range, strikes }
    }

    pub fn strikes(&self) -> &[(i32, Span)] {
        &self.strikes
    }

    fn facing(&self) -> f32 {
        match self.faction {
            Faction::Cat => -1.0,
            Faction::Enemy => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Combatant {
    pub profile: Arc<Profile>,
    pub attacking: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Line { from: Point, to: Point, color: Color },
    Box { corner: Point, size: Point, color: Color, fill: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab {
    pub corner: Point,
    pub size: Point,
    pub color: Color,
}

pub struct Pose<'a> {
    pub rig: &'a Rig,
    pub anim: Option<&'a Animation>,
    pub frame: i32,
    pub offset: Option<usize>,
    pub zoom: f32,
    pub crest: Option<f32>,
}

pub struct Seat {
    pub ground: Point,
    pub zoom: f32,
    pub bound: Option<f32>,
    pub crest: f32,
    pub spans_bound: bool,
    pub hurt: f32,
}

pub fn aged(period: Option<i32>, frame: f32, landed: i32) -> f32 {
    match (frame - landed as f32, period) {
        (age, Some(period)) if age < 0.0 => age + period as f32,
        (age, _) => age,
    }
}

pub fn ramp(fade: i32, age: f32) -> f32 {
    let life = fade.max(1) as f32;

    match (0.0..life).contains(&age) {
        true => 1.0 - age / life,
        false => 0.0,
    }
}

pub fn glow(fade: i32, period: Option<i32>, frame: f32, landed: i32) -> f32 {
    ramp(fade, aged(period, frame, landed))
}

fn rows(strikes: &[(i32, Span)], positions: HitPositions) -> Vec<(f32, f32)> {
    let count = strikes.len().min(STRIKE_COLORS.len());
    let clashing = |a: usize, b: usize| strikes[a].1.near < strikes[b].1.far && strikes[b].1.near < strikes[a].1.far;

    let mut group: Vec<usize> = (0..count).collect();

    if positions == HitPositions::Fill {
        for _ in 0..count {
            for a in 0..count {
                for b in 0..a {
                    if clashing(a, b) {
                        let joined = group[a].min(group[b]);

                        group[a] = joined;
                        group[b] = joined;
                    }
                }
            }
        }
    }

    (0..count)
        .map(|attack| match positions {
            HitPositions::Stacked => (0.0, 1.0),
            HitPositions::Layered => (attack as f32 / count as f32, 1.0 / count as f32),
            HitPositions::Fill => {
                let peers = group.iter().filter(|held| **held == group[attack]).count();
                let above = group[..attack].iter().filter(|held| **held == group[attack]).count();

                (above as f32 / peers as f32, 1.0 / peers as f32)
            }
        })
        .collect()
}

pub fn posed(
    profile: &Profile,
    look: CombatLook,
    pose: &Pose<'_>,
    to_screen: impl Fn(f32, f32) -> Point,
    lit: impl Fn(usize, i32) -> f32,
) -> Vec<Mark> {
    let anchors = animate::anchors(pose.rig, pose.anim, pose.frame);
    let anchored = |row: usize| anchors.get(row).copied().flatten();

    let Some(combat) = anchored(0) else {
        return Vec::new();
    };

    let lifted = pose.offset.and_then(anchored).unwrap_or((0.0, 0.0));
    let ground = to_screen(combat.0 - lifted.0, combat.1 - lifted.1);

    let bound = anchored(profile.faction.bound_row())
        .map(|(reach, _)| ground.0 - profile.facing() * profile.faction.bound_depth(reach) * pose.zoom);

    let crest = pose.crest.map_or(ground.1 - LINE_RISE * pose.zoom, |crest| to_screen(0.0, crest).1);

    let seat = Seat { ground, zoom: pose.zoom, bound, crest: crest.min(ground.1), spans_bound: true, hurt: 1.0 };

    arranged(profile, look, &seat, lit)
}

pub fn arranged(profile: &Profile, look: CombatLook, seat: &Seat, lit: impl Fn(usize, i32) -> f32) -> Vec<Mark> {
    let ground = seat.ground;
    let bound = seat.bound;

    let ahead = |distance: f32| ground.0 + profile.facing() * distance * seat.zoom;
    let depth = look.height * seat.zoom;
    let top = ground.1 - look.offset * seat.zoom;
    let boxed = |from: f32, to: f32, row: f32, share: f32, color: Color, fill: f32| Mark::Box {
        corner: (from.min(to), top + row * depth),
        size: ((to - from).abs(), share * depth),
        color,
        fill: fill * look.opacity,
    };
    let rising = |x: f32, color: Color| Mark::Line { from: (x, ground.1.max(top)), to: (x, seat.crest), color };

    let (left, right) = [ground.0, ahead(-profile.hurtbox.near), ahead(-profile.hurtbox.far), ahead(profile.range)]
        .into_iter()
        .chain(bound.filter(|_| seat.spans_bound))
        .chain(profile.strikes.iter().flat_map(|(_, reach)| [ahead(reach.near), ahead(reach.far)]))
        .fold((ground.0, ground.0), |(left, right), x| (left.min(x), right.max(x)));

    let mut marks = Vec::new();

    if look.system {
        marks.push(boxed(left, right, 0.0, 1.0, BASE_COLOR, 0.0));
    }

    if look.width {
        marks.push(boxed(ahead(-profile.hurtbox.near), ahead(-profile.hurtbox.far), 0.0, 1.0, HURTBOX_COLOR, seat.hurt));
    }

    marks.extend(bound.filter(|_| look.bound).map(|x| rising(x, BOUND_COLOR)));

    if look.range {
        marks.push(rising(ahead(profile.range), RANGE_COLOR));
    }

    if look.origin {
        marks.push(rising(ground.0, ORIGIN_COLOR));
    }

    let rows = rows(&profile.strikes, look.positions);

    for (attack, (((landed, reach), color), (row, share))) in
        profile.strikes.iter().zip(STRIKE_COLORS).zip(rows).enumerate().rev()
    {
        if !look.hits.get(attack).is_some_and(|shown| *shown) {
            continue;
        }

        marks.push(boxed(ahead(reach.near), ahead(reach.far), row, share, color, lit(attack, *landed)));
    }

    marks
}

pub fn slabs(marks: impl IntoIterator<Item = Mark>, stroke: f32) -> Vec<Slab> {
    let mut slabs = Vec::new();

    for mark in marks {
        match mark {
            Mark::Line { from, to, color } => {
                let width = LINE_WIDTH * stroke;

                slabs.push(Slab {
                    corner: (from.0.min(to.0) - width / 2.0, from.1.min(to.1)),
                    size: ((to.0 - from.0).abs() + width, (to.1 - from.1).abs()),
                    color,
                });
            }
            Mark::Box { corner, size, color, fill } => {
                let width = OUTLINE_WIDTH * stroke;
                let half = width / 2.0;

                if fill > 0.0 {
                    slabs.push(Slab { corner, size, color: [color[0], color[1], color[2], fill] });
                }

                let (left, top) = (corner.0 - half, corner.1 - half);
                let (wide, tall) = (size.0 + width, size.1 + width);

                slabs.extend([
                    Slab { corner: (left, top), size: (wide, width), color },
                    Slab { corner: (left, top + size.1), size: (wide, width), color },
                    Slab { corner: (left, top), size: (width, tall), color },
                    Slab { corner: (left + size.0, top), size: (width, tall), color },
                ]);
            }
        }
    }

    slabs
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fade that outlives the animation carries into the next pass, and along the
    // way the fill may only ever jump up on the frame its hit lands.
    #[test]
    fn a_fill_carries_across_the_loop_and_only_brightens_when_its_hit_lands() {
        let (fade, period, landed) = (30, 40, 12);
        let ticks: Vec<f32> = (0..period * 4).map(|tick| (tick % (period * 2)) as f32 / 2.0).collect();

        assert!(glow(fade, Some(period), 0.0, landed) > 0.0, "the last pass's tail is still lit");
        assert_eq!(glow(fade, None, 0.0, landed), 0.0, "and a first pass has no tail to show");

        for pair in ticks.windows(2) {
            let (before, after) = (glow(fade, Some(period), pair[0], landed), glow(fade, Some(period), pair[1], landed));

            assert!(after <= before || pair[1] == landed as f32, "rose at frame {}", pair[1]);
        }
    }

    fn span(near: f32, far: f32) -> (i32, Span) {
        (1, Span { near, far })
    }

    // Fill only splits the height between hits that actually share ground.
    #[test]
    fn fill_gives_a_lone_hit_the_whole_height_and_splits_it_between_hits_that_overlap() {
        let strikes = [span(0.0, 50.0), span(40.0, 90.0), span(200.0, 260.0)];

        assert_eq!(rows(&strikes, HitPositions::Fill), vec![(0.0, 0.5), (0.5, 0.5), (0.0, 1.0)]);
        assert_eq!(rows(&strikes, HitPositions::Stacked), vec![(0.0, 1.0); 3]);
        assert_eq!(rows(&strikes, HitPositions::Layered)[2], (2.0 / 3.0, 1.0 / 3.0));
    }

    #[test]
    fn hits_that_only_touch_at_an_edge_do_not_count_as_overlapping() {
        let strikes = [span(0.0, 50.0), span(50.0, 90.0)];

        assert_eq!(rows(&strikes, HitPositions::Fill), vec![(0.0, 1.0), (0.0, 1.0)]);
    }
}
