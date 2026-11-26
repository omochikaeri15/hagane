use std::collections::HashSet;

use crate::files::Param;

use super::arena::{RANGE_STEPS, WAVE_LEAD, WAVE_STRIDE};
use super::Entity;

const WHOLE: i32 = 100;
const HITS: usize = 3;
const WAVE_STEP: i32 = 4;
const WAVE_START: i32 = 3;
const WAVE_END: i32 = 7;
const WAVE_SPRITES: usize = 6;
const SURGE_DELAY: i32 = 15;
const SURGE_TICK: i32 = 20;
const SURGE_HALF: i32 = 750;
const SURGE_SHIFT: i32 = 250;
const RINGS: usize = 3;
const DIES_AFTER: i32 = 2;
const MOST_SWINGS: i32 = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub wave_frames: i32,
    pub mini_wave_frames: i32,
    pub mini_wave_step: i32,
    pub mini_wave_start: i32,
    pub mini_wave_end: i32,
    pub explosion_start: i32,
    pub explosion_window: i32,
    pub explosion_linger: i32,
    pub explosion_spread: i32,
    pub explosion_widths: [i32; RINGS],
}

impl Timing {
    pub fn new(param: &Param, wave_frames: i32, mini_wave_frames: i32) -> Self {
        Self {
            wave_frames,
            mini_wave_frames,
            mini_wave_step: param.mini_wave_interval(),
            mini_wave_start: param.mini_wave_first_frame(),
            mini_wave_end: param.mini_wave_last_frame(),
            explosion_start: param.explosion_start_time(),
            explosion_window: param.explosion_hitbox_duration(),
            explosion_linger: param.explosion_delay_before_deletion(),
            explosion_spread: param.explosion_delay_until_spike(),
            explosion_widths: [param.explosion_big_width(), param.explosion_medium_width(), param.explosion_small_width()],
        }
    }

    fn rings(&self) -> [Option<i32>; RINGS] {
        let spread = self.explosion_spread;
        let window = self.explosion_window;

        if spread <= 0 || window <= 0 {
            return [None; RINGS];
        }

        let erased = (self.explosion_start + 2 * spread + window + self.explosion_linger).max(1);

        std::array::from_fn(|ring| {
            let offset = spread * ring as i32;
            let opens = (self.explosion_start + offset).max(offset).max(1);

            (opens - offset < self.explosion_start + window && opens <= erased).then_some(opens - 1)
        })
    }

    fn passes(&self) -> Vec<(u8, i32, i32)> {
        let [first, second, third] = self.explosion_widths;
        let spreads = [0, first + second, first + 2 * second + third];

        (0..RINGS)
            .flat_map(|ring| {
                let width = self.explosion_widths[ring];
                let spread = spreads[ring];
                let centers = if ring == 0 { vec![0] } else { vec![-(spread / 2), spread - spread / 2] };

                centers.into_iter().map(move |center| (1u8 << ring, center - width / 2, center + width / 2 - 1))
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct Spawns {
    low: i32,
    high: i32,
}

impl Spawns {
    fn of(anchor: i32, span: i32) -> Self {
        let anchor = anchor.saturating_mul(RANGE_STEPS);
        let span = span.saturating_mul(RANGE_STEPS);
        let last = anchor.saturating_add(span.signum().saturating_mul((span.saturating_abs() - 1).max(0)));

        Self { low: anchor.min(last), high: anchor.max(last) }
    }

    fn total(self) -> i32 {
        self.high - self.low + 1
    }

    fn within(self, from: i32, to: i32) -> i32 {
        (to.min(self.high) - from.max(self.low) + 1).max(0)
    }

    fn turns(self, from: i32, to: i32) -> [i32; 4] {
        [self.low - from, self.high + 1 - from, self.low - 1 - to, self.high - to]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Profile {
    direct: [bool; HITS],
    wave: Option<i32>,
    surge: i32,
    blasts: Vec<(u8, i32)>,
}

struct Outcome {
    chance: f64,
    delays: Vec<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct Exposure {
    cycle: i32,
    hits: Vec<(i32, usize, bool)>,
    wave: f64,
    surge: f64,
    surge_spawns: i32,
    ticks: Vec<i32>,
    blast: f64,
    blast_spawns: i32,
    rings: [Option<i32>; RINGS],
    profiles: Vec<Profile>,
}

fn wave_steps(level: i32, step: i32, start: i32, end: i32, frames: i32) -> Vec<(i32, i32)> {
    if level <= 0 || step <= 0 || frames < 2 {
        return Vec::new();
    }

    let first = start.max(0);
    let lands = first <= end && first <= frames - 2;
    let mut spawned: Vec<i32> = Vec::new();
    let mut steps = Vec::new();

    for index in 1..=level.saturating_add(frames).saturating_add(1) {
        let spawn = index.saturating_mul(step) - 1;

        if spawned.iter().filter(|held| spawn - **held <= frames - 2).count() >= WAVE_SPRITES {
            continue;
        }

        spawned.push(spawn);

        if lands {
            steps.push((index, spawn + first));
        }

        if index >= level {
            break;
        }
    }

    steps
}

fn wave_front(index: i32) -> i32 {
    index * WAVE_STRIDE - WAVE_LEAD
}

fn chance_of(percent: i32) -> f64 {
    f64::from(percent.clamp(0, WHOLE)) / f64::from(WHOLE)
}

fn blasts(spawns: Spawns, position: i32, passes: &[(u8, i32, i32)]) -> Vec<(u8, i32)> {
    let mut marks = vec![spawns.low, spawns.high + 1];

    for (_, from, to) in passes {
        marks.extend([position + from, position + to + 1].map(|mark| mark.clamp(spawns.low, spawns.high + 1)));
    }

    marks.sort_unstable();
    marks.dedup();

    let mut counts: Vec<(u8, i32)> = Vec::new();

    for pair in marks.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let mask = passes
            .iter()
            .filter(|(_, low, high)| (position + low..=position + high).contains(&from))
            .fold(0u8, |mask, (bit, _, _)| mask | bit);

        if mask == 0 {
            continue;
        }

        match counts.iter_mut().find(|(held, _)| *held == mask) {
            Some((_, count)) => *count += to - from,
            None => counts.push((mask, to - from)),
        }
    }

    counts.sort_unstable();
    counts
}

fn moving(start: &Profile, end: &Profile) -> usize {
    let count = |blasts: &[(u8, i32)], mask: u8| blasts.iter().find(|(held, _)| *held == mask).map_or(0, |(_, count)| *count);
    let masks: HashSet<u8> = start.blasts.iter().chain(&end.blasts).map(|(mask, _)| *mask).collect();

    usize::from(start.surge != end.surge) + masks.into_iter().filter(|mask| count(&start.blasts, *mask) != count(&end.blasts, *mask)).count()
}

impl Entity {
    pub fn swings(&self) -> Option<i32> {
        (self.attack_count_state == DIES_AFTER && self.attack_count_total > -1).then_some(self.attack_count_total)
    }

    pub fn exposure(&self, cycle: i32, recharge: i32, timing: &Timing) -> Exposure {
        let flags = [self.attack_1_abilities, self.attack_2_abilities, self.attack_3_abilities];
        let strikes = self.strike_frames();
        let (period, swings) = self.swings().map_or((cycle, 1), |swings| (recharge, swings.clamp(0, MOST_SWINGS)));
        let hits: Vec<(i32, usize, bool)> = (0..swings)
            .flat_map(|swing| strikes.iter().zip(flags).enumerate().map(move |(attack, (frame, able))| (frame + swing * cycle, attack, able != 0)))
            .collect();
        let bands: Vec<(i32, i32)> = (0..strikes.len())
            .map(|attack| self.strike_band(attack))
            .map(|(near, far)| (near.saturating_mul(RANGE_STEPS), far.saturating_mul(RANGE_STEPS)))
            .collect();

        let mini = self.mini_wave_flag == 1;
        let steps = if self.wave_chance <= 0 {
            Vec::new()
        } else if mini {
            wave_steps(self.wave_level, timing.mini_wave_step, timing.mini_wave_start, timing.mini_wave_end, timing.mini_wave_frames)
        } else {
            wave_steps(self.wave_level, WAVE_STEP, WAVE_START, WAVE_END, timing.wave_frames)
        };

        let surge = Spawns::of(self.surge_spawn_anchor, self.surge_spawn_span);
        let ticks: Vec<i32> = (0..self.surge_level.max(0)).map(|tick| SURGE_DELAY - 1 + SURGE_TICK * tick).collect();
        let surging = self.surge_chance > 0 && !ticks.is_empty();
        let reach = (SURGE_SHIFT - SURGE_HALF, SURGE_SHIFT + SURGE_HALF - 1);

        let blast = Spawns::of(self.explosion_spawn_anchor, self.explosion_spawn_span);
        let rings = timing.rings();
        let passes: Vec<(u8, i32, i32)> = timing.passes().into_iter().filter(|(bit, _, _)| rings[bit.trailing_zeros() as usize].is_some()).collect();
        let blasting = self.explosion_chance > 0 && !passes.is_empty();

        let rolling: Vec<(i32, i32)> = bands.iter().zip(flags).filter(|(_, able)| *able != 0).map(|(band, _)| *band).collect();
        let inside = |position: i32| rolling.iter().any(|(near, far)| (*near..=*far).contains(&position));
        let profile = |position: i32| Profile {
            direct: std::array::from_fn(|attack| bands.get(attack).is_some_and(|(near, far)| (*near..=*far).contains(&position))),
            wave: steps.iter().find(|(index, _)| position > wave_front(*index) - WAVE_STRIDE && position <= wave_front(*index) + WAVE_STRIDE).map(|(_, delay)| *delay),
            surge: if surging { surge.within(position + reach.0, position + reach.1) } else { 0 },
            blasts: if blasting { blasts(blast, position, &passes) } else { Vec::new() },
        };

        let mut turns: Vec<i32> = Vec::new();

        for (near, far) in &rolling {
            turns.extend([*near, *far + 1]);
        }

        for (index, _) in &steps {
            turns.extend([wave_front(*index) - WAVE_STRIDE + 1, wave_front(*index) + WAVE_STRIDE + 1]);
        }

        if surging {
            turns.extend(surge.turns(reach.0, reach.1));
        }

        if blasting {
            for (_, from, to) in &passes {
                turns.extend(blast.turns(*from, *to));
            }
        }

        let mut marks: Vec<i32> = turns.iter().flat_map(|turn| [turn - 1, *turn, turn + 1]).filter(|mark| inside(*mark)).collect();

        marks.sort_unstable();
        marks.dedup();

        let mut seen: HashSet<Profile> = HashSet::new();
        let mut profiles = Vec::new();
        let mut keep = |held: Profile| {
            if seen.insert(held.clone()) {
                profiles.push(held);
            }
        };

        for pair in marks.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let (start, end) = (profile(from), profile(to));
            if moving(&start, &end) > 1 && to - from > 1 && inside(from + 1) {
                for position in from + 1..to {
                    keep(profile(position));
                }
            }

            keep(start);
        }

        if let Some(last) = marks.last() {
            keep(profile(*last));
        }

        Exposure {
            cycle: period,
            hits,
            wave: if steps.is_empty() { 0.0 } else { chance_of(self.wave_chance) },
            surge: if surging { chance_of(self.surge_chance) } else { 0.0 },
            surge_spawns: surge.total(),
            ticks,
            blast: if blasting { chance_of(self.explosion_chance) } else { 0.0 },
            blast_spawns: blast.total(),
            rings,
            profiles,
        }
    }
}

impl Exposure {
    pub fn uptime(&self, chance: i32, duration: i32) -> Option<(f64, f64)> {
        let lasting = duration - 1;

        if self.cycle <= 0 || lasting <= 0 || self.profiles.is_empty() {
            return None;
        }

        let chance = chance_of(chance);
        let least = self.profiles.iter().map(|profile| self.share(profile, chance, lasting, false)).fold(f64::INFINITY, f64::min);
        let most = self.profiles.iter().map(|profile| self.share(profile, chance, lasting, true)).fold(0.0, f64::max);

        Some((least, most))
    }

    fn outcomes(&self, profile: &Profile) -> Vec<Vec<Outcome>> {
        let mut groups = Vec::new();

        if let Some(delay) = profile.wave.filter(|_| self.wave > 0.0) {
            groups.push(vec![Outcome { chance: self.wave, delays: vec![delay] }]);
        }

        if profile.surge > 0 && self.surge > 0.0 {
            groups.push(vec![Outcome { chance: self.surge * f64::from(profile.surge) / f64::from(self.surge_spawns), delays: self.ticks.clone() }]);
        }

        if self.blast > 0.0 && !profile.blasts.is_empty() {
            groups.push(
                profile
                    .blasts
                    .iter()
                    .map(|(mask, count)| Outcome {
                        chance: self.blast * f64::from(*count) / f64::from(self.blast_spawns),
                        delays: (0..RINGS).filter(|ring| mask & (1 << ring) != 0).filter_map(|ring| self.rings[ring]).collect(),
                    })
                    .collect(),
            );
        }

        groups
    }

    fn share(&self, profile: &Profile, chance: f64, lasting: i32, extras: bool) -> f64 {
        let shared = if extras { self.outcomes(profile) } else { Vec::new() };
        let latest = shared.iter().flatten().flat_map(|outcome| outcome.delays.iter().copied()).max().unwrap_or(0).max(0);
        let reach = (latest + lasting) as usize;
        let held = |delays: &[i32], gap: i32| delays.iter().any(|delay| gap > *delay && gap <= delay + lasting);
        let table = |direct: bool| -> Vec<f64> {
            (0..=reach as i32)
                .map(|gap| {
                    let missed = shared
                        .iter()
                        .map(|group| 1.0 - group.iter().filter(|outcome| held(&outcome.delays, gap)).map(|outcome| outcome.chance).sum::<f64>())
                        .product::<f64>();
                    let missed = if direct && held(&[0], gap) { 0.0 } else { missed };

                    1.0 - missed
                })
                .collect()
        };
        let struck = table(true);
        let spared = table(false);
        let cycle = self.cycle;

        let covered: f64 = (0..cycle)
            .map(|frame| {
                let missed: f64 = self
                    .hits
                    .iter()
                    .filter(|(_, _, able)| *able)
                    .map(|(offset, attack, _)| {
                        let table = if profile.direct[*attack] { &struck } else { &spared };
                        let mut gap = (frame - offset).rem_euclid(cycle);

                        if gap == 0 {
                            gap = cycle;
                        }

                        let mut missed = 1.0;

                        while gap as usize <= reach {
                            missed *= 1.0 - chance * table[gap as usize];
                            gap += cycle;
                        }

                        missed
                    })
                    .product();

                1.0 - missed
            })
            .sum();

        covered / f64::from(cycle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: Timing = Timing {
        wave_frames: 13,
        mini_wave_frames: 13,
        mini_wave_step: 2,
        mini_wave_start: 2,
        mini_wave_end: 5,
        explosion_start: 11,
        explosion_window: 15,
        explosion_linger: 0,
        explosion_spread: 10,
        explosion_widths: [600, 400, 400],
    };

    fn close(value: f64, expected: f64) -> bool {
        (value - expected).abs() < 1e-9
    }

    fn freezer(chance: i32) -> Entity {
        Entity { standing_range: 400, time_until_attack_1: 10, attack_1_abilities: 1, freeze_chance: chance, ..Entity::default() }
    }

    // The timer is set after the target has moved that frame, so a proc holds it one frame less
    // than its duration; a duration one longer than the cycle never lets go.
    #[test]
    fn a_proc_holds_one_frame_less_than_its_duration() {
        let exposure = freezer(100).exposure(100, 0, &TIMING);

        assert_eq!(exposure.uptime(100, 101).unwrap(), (1.0, 1.0));
        assert_eq!(exposure.uptime(100, 100).unwrap(), (0.99, 0.99));
        assert!(close(exposure.uptime(50, 201).unwrap().0, 0.75), "two cycles cover every frame, missed only when both rolls fail");
        assert_eq!(exposure.uptime(100, 1), None, "a timer that runs out on the frame it is set holds nothing");
    }

    // Each hit of a multi-hit rolls on its own, so where their windows overlap a frame is only missed
    // when every roll fails, and a hit without the ability flag never rolls at all.
    #[test]
    fn multi_hits_overlap_by_their_spacing() {
        let both = |second: i32| Entity { time_until_attack_2: second, attack_2_abilities: 1, ..freezer(50) };

        assert!(close(both(60).exposure(100, 0, &TIMING).uptime(50, 51).unwrap().0, 0.5));
        assert!(close(both(20).exposure(100, 0, &TIMING).uptime(50, 51).unwrap().0, 0.5 * 0.1 + 0.75 * 0.4 + 0.5 * 0.1));
        assert!(close(Entity { attack_2_abilities: 0, ..both(20) }.exposure(100, 0, &TIMING).uptime(50, 51).unwrap().0, 0.5 * 0.5));
    }

    // A wave carries the roll of the hit that made it, landing a few frames later where it reaches,
    // and the mini wave steps out twice as fast; an immune foe only gets the hit itself.
    #[test]
    fn waves_land_later_and_mini_waves_sooner() {
        let wave = |mini_wave_flag: i32| Entity { wave_chance: 100, wave_level: 1, mini_wave_flag, ..freezer(100) };

        assert_eq!(wave_steps(1, WAVE_STEP, WAVE_START, WAVE_END, 13), vec![(1, 6)]);
        assert_eq!(wave_steps(1, 2, 2, 5, 13), vec![(1, 3)]);

        let (least, most) = wave(0).exposure(100, 0, &TIMING).uptime(100, 31).unwrap();
        assert!(close(least, 0.3) && close(most, 0.36));

        let (_, most) = wave(1).exposure(100, 0, &TIMING).uptime(100, 31).unwrap();
        assert!(close(most, 0.33));
    }

    // Six sprites are all a wave has, and a step that finds none free is skipped, never queued.
    #[test]
    fn a_long_wave_animation_skips_steps() {
        assert_eq!(wave_steps(4, 2, 2, 5, 13).len(), 4);
        assert_eq!(wave_steps(8, 2, 2, 5, 15).iter().map(|(index, _)| *index).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6, 8], "the seventh step finds every sprite busy");
    }

    // A surge spawns somewhere along its span, so a spot only counts as often as one lands on it,
    // and the best spot inside the attack range sets the most.
    #[test]
    fn surges_count_by_where_they_spawn() {
        let surge = Entity { surge_chance: 100, surge_level: 1, surge_spawn_anchor: 300, surge_spawn_span: 750, ..freezer(100) };
        let exposure = surge.exposure(100, 0, &TIMING);
        let (least, most) = exposure.uptime(100, 31).unwrap();
        let spot = 4 * 400;
        let landed = Spawns::of(300, 750).within(spot - 500, spot + 999);

        assert!(close(least, 0.3));
        assert!(close(most, 0.3 + 0.14 * f64::from(landed) / 3000.0), "the far end of the range sees the most surges");
    }

    // A unit that dies after its attacks only comes back once it recharges, so its rolls repeat on
    // the recharge instead of the attack cycle.
    #[test]
    fn a_unit_that_dies_repeats_on_its_recharge() {
        let bomber = Entity { attack_count_total: 1, attack_count_state: DIES_AFTER, ..freezer(100) };

        assert!(close(bomber.exposure(100, 300, &TIMING).uptime(100, 31).unwrap().0, 0.1));
        assert!(close(Entity { attack_count_total: 2, ..bomber.clone() }.exposure(100, 300, &TIMING).uptime(100, 31).unwrap().0, 0.2), "both swings land before it goes");
        assert_eq!(bomber.exposure(100, 0, &TIMING).uptime(100, 31), None, "no recharge, nothing to measure by");
    }

    // A hit that cannot roll never holds the foe, so the spots only it reaches are no place to measure;
    // here the first hit is plain and only the second, with its own band, freezes.
    #[test]
    fn only_hits_that_roll_decide_where_the_foe_stands() {
        let split = Entity {
            long_distance_1_anchor: 1,
            long_distance_1_span: 400,
            time_until_attack_2: 60,
            attack_1_abilities: 0,
            attack_2_abilities: 1,
            long_distance_2_flag: 1,
            long_distance_2_anchor: 100,
            long_distance_2_span: 900,
            ..freezer(100)
        };

        assert_eq!(split.exposure(100, 0, &TIMING).uptime(100, 31), Some((0.3, 0.3)));
        assert_eq!(Entity { attack_2_abilities: 0, ..split }.exposure(100, 0, &TIMING).uptime(100, 31), None);
    }

    // An explosion goes off at its spawn point and reaches a foe with the ring it stands in.
    #[test]
    fn explosions_land_with_their_ring() {
        assert_eq!(TIMING.rings(), [Some(10), Some(20), Some(30)]);

        let bomb = Entity { explosion_chance: 100, explosion_spawn_anchor: 100, ..freezer(100) };
        let (least, most) = bomb.exposure(100, 0, &TIMING).uptime(100, 31).unwrap();

        assert!(close(least, 0.3));
        assert!(close(most, 0.6), "the outer ring lands 30 frames after the hit");
    }
}
