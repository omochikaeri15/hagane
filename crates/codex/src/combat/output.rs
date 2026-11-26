use crate::files::Param;

use super::arena::{RANGE_STEPS, WAVE_LEAD, WAVE_STRIDE};
use super::versus::{Carrier, Foe, Roll, Terms};
use super::{Entity, Faction};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    Minimum,
    #[default]
    Average,
    Maximum,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Hits {
    #[default]
    Standing,
    All,
    Only(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rules {
    pub wave_frames: i32,
    pub mini_wave_frames: i32,
    pub mini_wave_step: i32,
    pub mini_wave_start: i32,
    pub mini_wave_end: i32,
    pub mini_wave_damage: i32,
    pub explosion_start: i32,
    pub explosion_window: i32,
    pub explosion_linger: i32,
    pub explosion_spread: i32,
    pub explosion_widths: [i32; RINGS],
    pub explosion_damages: [i32; RINGS],
}

impl Rules {
    pub fn new(param: &Param, wave_frames: i32, mini_wave_frames: i32) -> Self {
        Self {
            wave_frames,
            mini_wave_frames,
            mini_wave_step: param.mini_wave_interval(),
            mini_wave_start: param.mini_wave_first_frame(),
            mini_wave_end: param.mini_wave_last_frame(),
            mini_wave_damage: param.mini_wave_damage(),
            explosion_start: param.explosion_start_time(),
            explosion_window: param.explosion_hitbox_duration(),
            explosion_linger: param.explosion_delay_before_deletion(),
            explosion_spread: param.explosion_delay_until_spike(),
            explosion_widths: [param.explosion_big_width(), param.explosion_medium_width(), param.explosion_small_width()],
            explosion_damages: [param.explosion_big_damage_multiplier(), param.explosion_medium_damage_multiplier(), param.explosion_small_damage_multiplier()],
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
}

struct Outcome {
    chance: f64,
    delays: Vec<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct Exposure {
    period: i32,
    swings: Vec<(i32, usize)>,
    able: [bool; HITS],
    lands: [bool; HITS],
    critical: f64,
    savage: f64,
    wave: Option<i32>,
    wave_carrier: Option<Carrier>,
    wave_chance: f64,
    surge: f64,
    surge_carrier: Option<Carrier>,
    surge_chance: f64,
    ticks: Vec<i32>,
    blasts: Vec<(u8, f64)>,
    blast_chance: f64,
    rings: [Option<i32>; RINGS],
    damages: [i32; RINGS],
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

fn wave_covers(faction: Faction, index: i32, spot: i32) -> bool {
    let front = index * WAVE_STRIDE;

    match faction {
        Faction::Cat => spot > front - WAVE_LEAD - WAVE_STRIDE && spot <= front - WAVE_LEAD + WAVE_STRIDE,
        Faction::Enemy => spot >= front + WAVE_LEAD - WAVE_STRIDE && spot < front + WAVE_LEAD + WAVE_STRIDE,
    }
}

fn surge_reach(faction: Faction, spot: i32) -> (i32, i32) {
    match faction {
        Faction::Cat => (spot - SURGE_HALF + SURGE_SHIFT, spot + SURGE_HALF + SURGE_SHIFT - 1),
        Faction::Enemy => (spot - SURGE_HALF + SURGE_SHIFT + 1, spot + SURGE_HALF + SURGE_SHIFT),
    }
}

fn blast_reach(faction: Faction, spot: i32, from: i32, to: i32) -> (i32, i32) {
    match faction {
        Faction::Cat => (spot + from, spot + to),
        Faction::Enemy => (spot - to, spot - from),
    }
}

fn chance_of(percent: i32) -> f64 {
    f64::from(percent.clamp(0, WHOLE)) / f64::from(WHOLE)
}

fn blasts(spawns: Spawns, reaches: &[(u8, i32, i32)]) -> Vec<(u8, f64)> {
    let mut marks = vec![spawns.low, spawns.high + 1];

    for (_, from, to) in reaches {
        marks.extend([*from, to + 1].map(|mark| mark.clamp(spawns.low, spawns.high + 1)));
    }

    marks.sort_unstable();
    marks.dedup();

    let mut counts: Vec<(u8, i32)> = Vec::new();

    for pair in marks.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let mask = reaches.iter().filter(|(_, low, high)| (*low..=*high).contains(&from)).fold(0u8, |mask, (bit, _, _)| mask | bit);

        if mask == 0 {
            continue;
        }

        match counts.iter_mut().find(|(held, _)| *held == mask) {
            Some((_, count)) => *count += to - from,
            None => counts.push((mask, to - from)),
        }
    }

    counts.into_iter().map(|(mask, count)| (mask, f64::from(count) / f64::from(spawns.total()))).collect()
}

impl Entity {
    pub fn swings(&self) -> Option<i32> {
        (self.attack_count_state == DIES_AFTER && self.attack_count_total > -1).then_some(self.attack_count_total)
    }

    pub fn exposure(&self, cycle: i32, recharge: i32, hits: Hits, rules: &Rules) -> Exposure {
        let flags = [self.attack_1_abilities, self.attack_2_abilities, self.attack_3_abilities];
        let strikes = self.strike_frames();
        let bands: Vec<(i32, i32)> = (0..strikes.len())
            .map(|attack| self.strike_band(attack))
            .map(|(near, far)| (near.saturating_mul(RANGE_STEPS), far.saturating_mul(RANGE_STEPS)))
            .collect();
        let standing = self.standing_range.saturating_mul(RANGE_STEPS);
        let (spot, counted, forced) = match hits {
            Hits::Only(attack) if attack < bands.len() => (standing.clamp(bands[attack].0, bands[attack].1.max(bands[attack].0)), vec![attack], true),
            Hits::All => (standing, (0..bands.len()).collect(), true),
            _ => (standing, (0..bands.len()).collect::<Vec<_>>(), false),
        };
        let lands: [bool; HITS] = std::array::from_fn(|attack| counted.contains(&attack) && (forced || bands.get(attack).is_some_and(|(near, far)| (*near..=*far).contains(&spot))));
        let able: [bool; HITS] = std::array::from_fn(|attack| flags[attack] != 0);

        let (period, swings) = self.swings().map_or((cycle, 1), |swings| (recharge, swings.clamp(0, MOST_SWINGS)));
        let frames = &strikes;
        let swings: Vec<(i32, usize)> = (0..swings).flat_map(|swing| counted.iter().map(move |attack| (frames[*attack] + swing * cycle, *attack))).collect();

        let mini = self.mini_wave_flag == 1;
        let steps = if self.wave_chance <= 0 {
            Vec::new()
        } else if mini {
            wave_steps(self.wave_level, rules.mini_wave_step, rules.mini_wave_start, rules.mini_wave_end, rules.mini_wave_frames)
        } else {
            wave_steps(self.wave_level, WAVE_STEP, WAVE_START, WAVE_END, rules.wave_frames)
        };
        let wave = steps.iter().find(|(index, _)| wave_covers(self.faction, *index, spot)).map(|(_, delay)| *delay);

        let ticks: Vec<i32> = (0..self.surge_level.max(0)).map(|tick| SURGE_DELAY - 1 + SURGE_TICK * tick).collect();
        let surging = self.surge_chance > 0 && !ticks.is_empty();
        let surge_spawns = Spawns::of(self.surge_spawn_anchor, self.surge_spawn_span);
        let (from, to) = surge_reach(self.faction, spot);
        let surge = if surging { f64::from(surge_spawns.within(from, to)) / f64::from(surge_spawns.total()) } else { 0.0 };

        let rings = rules.rings();
        let reaches: Vec<(u8, i32, i32)> = rules
            .passes()
            .into_iter()
            .filter(|(bit, _, _)| rings[bit.trailing_zeros() as usize].is_some())
            .map(|(bit, from, to)| {
                let (low, high) = blast_reach(self.faction, spot, from, to);

                (bit, low, high)
            })
            .collect();
        let blasting = self.explosion_chance > 0 && !reaches.is_empty();

        Exposure {
            period,
            swings,
            able,
            lands,
            critical: chance_of(self.critical_chance),
            savage: chance_of(self.savage_blow_chance),
            wave,
            wave_carrier: Some(if mini { Carrier::MiniWave(rules.mini_wave_damage) } else { Carrier::Wave }),
            wave_chance: chance_of(self.wave_chance),
            surge,
            surge_carrier: Some(if self.mini_surge_flag != 0 { Carrier::MiniSurge } else { Carrier::Surge }),
            surge_chance: if surging { chance_of(self.surge_chance) } else { 0.0 },
            ticks,
            blasts: if blasting { blasts(Spawns::of(self.explosion_spawn_anchor, self.explosion_spawn_span), &reaches) } else { Vec::new() },
            blast_chance: if blasting { chance_of(self.explosion_chance) } else { 0.0 },
            rings,
            damages: rules.explosion_damages,
        }
    }
}

impl Exposure {
    fn counted(&self) -> Vec<usize> {
        let mut counted: Vec<usize> = self.swings.iter().map(|(_, attack)| *attack).collect();

        counted.sort_unstable();
        counted.dedup();
        counted
    }

    fn ringed(&self, mask: u8) -> Vec<usize> {
        (0..RINGS).filter(|ring| mask & (1 << ring) != 0 && self.rings[*ring].is_some()).collect()
    }

    pub fn damage(&self, stats: &Entity, attacks: [i32; HITS], foe: Foe, terms: Terms, mode: Mode) -> f64 {
        let mut total = 0.0;

        for attack in self.counted() {
            let able = self.able[attack];
            let (critical, savage) = match mode {
                Mode::Minimum => (0.0, 0.0),
                _ if !able => (0.0, 0.0),
                Mode::Average => (self.critical, self.savage),
                Mode::Maximum => (f64::from(u8::from(self.critical > 0.0)), f64::from(u8::from(self.savage > 0.0))),
            };
            let expect = |carrier: Carrier| -> f64 {
                [(false, false), (false, true), (true, false), (true, true)]
                    .into_iter()
                    .map(|(crit, sav)| {
                        let odds = if crit { critical } else { 1.0 - critical } * if sav { savage } else { 1.0 - savage };

                        odds * stats.landing(foe, terms, attacks[attack], carrier, Roll { critical: crit, savage: sav, boosted: mode == Mode::Maximum }) as f64
                    })
                    .sum()
            };

            if self.lands[attack] {
                total += expect(Carrier::Direct);
            }

            if mode == Mode::Minimum || !able {
                continue;
            }

            let most = mode == Mode::Maximum;

            if let (Some(_), Some(carrier)) = (self.wave, self.wave_carrier) {
                total += if most { 1.0 } else { self.wave_chance } * expect(carrier);
            }

            if let (true, Some(carrier)) = (self.surge > 0.0, self.surge_carrier) {
                total += if most { 1.0 } else { self.surge_chance * self.surge } * self.ticks.len() as f64 * expect(carrier);
            }

            let blast = |mask: u8| self.ringed(mask).into_iter().map(|ring| expect(Carrier::Explosion(self.damages[ring]))).sum::<f64>();

            total += if most {
                self.blasts.iter().map(|(mask, _)| blast(*mask)).fold(0.0, f64::max)
            } else {
                self.blasts.iter().map(|(mask, share)| self.blast_chance * share * blast(*mask)).sum()
            };
        }

        total
    }

    pub fn uptime(&self, chance: i32, duration: i32, mode: Mode) -> Option<f64> {
        let lasting = duration - 1;

        if self.period <= 0 || lasting <= 0 || self.swings.is_empty() {
            return None;
        }

        let rolled = match mode {
            Mode::Maximum => f64::from(u8::from(chance > 0)),
            _ => chance_of(chance),
        };

        Some(match mode {
            Mode::Minimum => self.share(&[], rolled, lasting),
            Mode::Average => self.share(&self.outcomes(None), rolled, lasting),
            Mode::Maximum if self.blasts.is_empty() => self.share(&self.outcomes(Some(0)), rolled, lasting),
            Mode::Maximum => self.blasts.iter().map(|(mask, _)| self.share(&self.outcomes(Some(*mask)), rolled, lasting)).fold(0.0, f64::max),
        })
    }

    fn outcomes(&self, best: Option<u8>) -> Vec<Vec<Outcome>> {
        let most = best.is_some();
        let mut groups = Vec::new();

        if let Some(delay) = self.wave {
            groups.push(vec![Outcome { chance: if most { 1.0 } else { self.wave_chance }, delays: vec![delay] }]);
        }

        if self.surge > 0.0 {
            groups.push(vec![Outcome { chance: if most { 1.0 } else { self.surge_chance * self.surge }, delays: self.ticks.clone() }]);
        }

        let delays = |mask: u8| -> Vec<i32> { self.ringed(mask).into_iter().filter_map(|ring| self.rings[ring]).collect() };

        match best {
            Some(mask) if mask != 0 => groups.push(vec![Outcome { chance: 1.0, delays: delays(mask) }]),
            Some(_) => {}
            None if !self.blasts.is_empty() => groups.push(self.blasts.iter().map(|(mask, share)| Outcome { chance: self.blast_chance * share, delays: delays(*mask) }).collect()),
            None => {}
        }

        groups
    }

    fn share(&self, shared: &[Vec<Outcome>], chance: f64, lasting: i32) -> f64 {
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

                    if direct && held(&[0], gap) { 1.0 } else { 1.0 - missed }
                })
                .collect()
        };
        let struck = table(true);
        let spared = table(false);
        let period = self.period;

        let covered: f64 = (0..period)
            .map(|frame| {
                let missed: f64 = self
                    .swings
                    .iter()
                    .filter(|(_, attack)| self.able[*attack])
                    .map(|(offset, attack)| {
                        let table = if self.lands[*attack] { &struck } else { &spared };
                        let mut gap = (frame - offset).rem_euclid(period);

                        if gap == 0 {
                            gap = period;
                        }

                        let mut missed = 1.0;

                        while gap as usize <= reach {
                            missed *= 1.0 - chance * table[gap as usize];
                            gap += period;
                        }

                        missed
                    })
                    .product();

                1.0 - missed
            })
            .sum();

        covered / f64::from(period)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Identity;

    const RULES: Rules = Rules {
        wave_frames: 13,
        mini_wave_frames: 13,
        mini_wave_step: 2,
        mini_wave_start: 2,
        mini_wave_end: 5,
        mini_wave_damage: 20,
        explosion_start: 11,
        explosion_window: 15,
        explosion_linger: 0,
        explosion_spread: 10,
        explosion_widths: [600, 400, 400],
        explosion_damages: [100, 70, 40],
    };

    fn close(value: f64, expected: f64) -> bool {
        (value - expected).abs() < 1e-9
    }

    fn freezer(chance: i32) -> Entity {
        Entity { standing_range: 300, time_until_attack_1: 10, attack_1_abilities: 1, freeze_chance: chance, ..Entity::default() }
    }

    fn uptime(unit: &Entity, chance: i32, duration: i32, mode: Mode) -> f64 {
        unit.exposure(100, 0, Hits::Standing, &RULES).uptime(chance, duration, mode).unwrap_or(-1.0)
    }

    // The timer is set after the target has moved that frame, so a proc holds it one frame less
    // than its duration; a duration one longer than the cycle never lets go.
    #[test]
    fn a_proc_holds_one_frame_less_than_its_duration() {
        let unit = freezer(100);

        assert_eq!(uptime(&unit, 100, 101, Mode::Average), 1.0);
        assert_eq!(uptime(&unit, 100, 100, Mode::Average), 0.99);
        assert!(close(uptime(&unit, 50, 201, Mode::Average), 0.75), "two cycles cover every frame, missed only when both rolls fail");
        assert_eq!(uptime(&unit, 50, 51, Mode::Maximum), 0.5, "at most every roll lands");
        assert_eq!(unit.exposure(100, 0, Hits::Standing, &RULES).uptime(100, 1, Mode::Average), None);
    }

    // Each hit of a multi-hit rolls on its own, so where their windows overlap a frame is only missed
    // when every roll fails, and a hit without the ability flag never rolls at all.
    #[test]
    fn multi_hits_overlap_by_their_spacing() {
        let both = |second: i32| Entity { time_until_attack_2: second, attack_2_abilities: 1, ..freezer(50) };

        assert!(close(uptime(&both(60), 50, 51, Mode::Average), 0.5));
        assert!(close(uptime(&both(20), 50, 51, Mode::Average), 0.5 * 0.1 + 0.75 * 0.4 + 0.5 * 0.1));
        assert!(close(uptime(&Entity { attack_2_abilities: 0, ..both(20) }, 50, 51, Mode::Average), 0.5 * 0.5));
    }

    // A wave carries the roll of the hit that made it, landing a few frames later, and the mini wave
    // steps out twice as fast; the minimum leaves every wave, surge and explosion out.
    #[test]
    fn waves_land_later_and_mini_waves_sooner() {
        let wave = |mini_wave_flag: i32| Entity { wave_chance: 100, wave_level: 1, mini_wave_flag, ..freezer(100) };

        assert_eq!(wave_steps(1, WAVE_STEP, WAVE_START, WAVE_END, 13), vec![(1, 6)]);
        assert_eq!(wave_steps(1, 2, 2, 5, 13), vec![(1, 3)]);
        assert!(close(uptime(&wave(0), 100, 31, Mode::Minimum), 0.3));
        assert!(close(uptime(&wave(0), 100, 31, Mode::Average), 0.36));
        assert!(close(uptime(&wave(1), 100, 31, Mode::Average), 0.33));
        assert!(close(uptime(&Entity { standing_range: 400, ..wave(0) }, 100, 31, Mode::Average), 0.3), "a level 1 wave stops short of 400");
    }

    // Six sprites are all a wave has, and a step that finds none free is skipped, never queued.
    #[test]
    fn a_long_wave_animation_skips_steps() {
        assert_eq!(wave_steps(4, 2, 2, 5, 13).len(), 4);
        assert_eq!(wave_steps(8, 2, 2, 5, 15).iter().map(|(index, _)| *index).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6, 8], "the seventh step finds every sprite busy");
    }

    // A surge spawns somewhere along its span, so at standing range it counts only as often as one
    // lands there; at most it always does.
    #[test]
    fn surges_count_by_where_they_spawn() {
        let surge = Entity { surge_chance: 100, surge_level: 1, surge_spawn_anchor: 300, surge_spawn_span: 750, ..freezer(100) };
        let spot = 4 * 300;
        let landed = Spawns::of(300, 750).within(spot - 500, spot + 999);

        assert!(close(uptime(&surge, 100, 31, Mode::Average), 0.3 + 0.14 * f64::from(landed) / 3000.0));
        assert!(close(uptime(&surge, 100, 31, Mode::Maximum), 0.44));
    }

    // An explosion goes off at its spawn point and reaches a foe with the ring it stands in.
    #[test]
    fn explosions_land_with_their_ring() {
        assert_eq!(RULES.rings(), [Some(10), Some(20), Some(30)]);

        let bomb = |standing_range: i32| Entity { explosion_chance: 100, explosion_spawn_anchor: 100, standing_range, ..freezer(100) };

        assert!(close(uptime(&bomb(150), 100, 31, Mode::Average), 0.4), "the inner ring lands 10 frames after the hit");
        assert!(close(uptime(&bomb(300), 100, 31, Mode::Average), 0.6), "the outer ring lands 30 frames after it");
    }

    // A unit that dies after its attacks only comes back once it recharges, so its rolls repeat on
    // the recharge instead of the attack cycle.
    #[test]
    fn a_unit_that_dies_repeats_on_its_recharge() {
        let bomber = Entity { attack_count_total: 1, attack_count_state: DIES_AFTER, ..freezer(100) };
        let pace = |unit: &Entity, recharge: i32| unit.exposure(100, recharge, Hits::Standing, &RULES).uptime(100, 31, Mode::Average);

        assert_eq!(pace(&bomber, 300).map(|share| close(share, 0.1)), Some(true));
        assert_eq!(pace(&Entity { attack_count_total: 2, ..bomber.clone() }, 300).map(|share| close(share, 0.2)), Some(true), "both swings land before it goes");
        assert_eq!(pace(&bomber, 0), None, "no recharge, nothing to measure by");
    }

    // At standing range only the hits whose band reaches it land; a picked hit lands wherever its band is,
    // and All Hits lands every one.
    #[test]
    fn the_hit_picker_decides_what_lands() {
        let split = Entity {
            standing_range: 400,
            attack_1_damage: 1000,
            attack_2_damage: 3000,
            time_until_attack_2: 60,
            attack_2_abilities: 1,
            long_distance_1_anchor: 500,
            long_distance_1_span: 100,
            long_distance_2_flag: 1,
            long_distance_2_anchor: 100,
            long_distance_2_span: 900,
            ..freezer(100)
        };
        let dealt = |hits: Hits| split.exposure(100, 0, hits, &RULES).damage(&split, [1000, 3000, 0], Foe::default(), Terms::default(), Mode::Minimum);

        assert_eq!(dealt(Hits::Standing), 3000.0, "the first hit's band starts past standing range");
        assert_eq!(dealt(Hits::All), 4000.0);
        assert_eq!(dealt(Hits::Only(0)), 1000.0);
        assert_eq!(dealt(Hits::Only(1)), 3000.0);
    }

    // Against Metal every landing that is not a critical hit deals 1, so averages are taken per landing,
    // and a hit without the ability flag can never be critical.
    #[test]
    fn metal_averages_each_landing() {
        let metal = Foe::default().with(Identity::TraitMetal);
        let mixed = Entity { attack_1_damage: 3000, attack_2_damage: 1000, time_until_attack_2: 20, attack_1_abilities: 1, attack_2_abilities: 0, critical_chance: 50, ..freezer(0) };
        let dealt = |unit: &Entity, mode: Mode| unit.exposure(100, 0, Hits::Standing, &RULES).damage(unit, [unit.attack_1_damage, unit.attack_2_damage, 0], metal, Terms::default(), mode);

        assert_eq!((dealt(&mixed, Mode::Minimum), dealt(&mixed, Mode::Average), dealt(&mixed, Mode::Maximum)), (2.0, 3001.5, 6001.0));

        let savage = Entity { attack_1_damage: 1000, critical_chance: 20, savage_blow_chance: 30, savage_blow_boost: 200, ..freezer(0) };

        assert!(close(dealt(&savage, Mode::Average), 0.2 * 0.3 * 6000.0 + 0.2 * 0.7 * 2000.0 + 0.8));
        assert_eq!(dealt(&savage, Mode::Maximum), 6000.0);

        let strong = Entity { attack_1_damage: 1000, strengthen_threshold: 50, strengthen_boost: 100, ..freezer(0) };

        assert_eq!((dealt(&strong, Mode::Average), dealt(&strong, Mode::Maximum)), (1.0, 1.0), "against Metal even Strengthen is held to 1");
        assert_eq!(strong.exposure(100, 0, Hits::Standing, &RULES).damage(&strong, [1000, 0, 0], Foe::default(), Terms::default(), Mode::Maximum), 2000.0, "only the maximum counts Strengthen");

        let mini = Entity { attack_1_damage: 1000, critical_chance: 50, wave_chance: 100, wave_level: 1, mini_wave_flag: 1, ..freezer(0) };

        assert!(close(dealt(&mini, Mode::Average), 1000.5 + 200.5));
        assert_eq!((dealt(&mini, Mode::Minimum), dealt(&mini, Mode::Maximum)), (1.0, 2400.0));
    }
}
