use codex::combat::{Entity, Faction, Foe, Hits, Identity, Mode, Rules, Switch, Switches, Terms, REGISTRY};

use crate::common::context::GlobalContext;
use crate::common::decimal::trimmed;
use crate::systems::combat::abilities::collect_ability_data_against;
use crate::systems::combat::registry::{Rhythm, StatContext, STAT_ATK_CYCLE, STAT_COOLDOWN};
use crate::systems::combat::{AbilityGroups, RenderContext};

const FRAMES_PER_SECOND: f64 = 30.0;

pub const MODES: [Mode; 3] = [Mode::Base, Mode::Average, Mode::Maximum];
pub const ONE_HIT: &str = "This unit only has one hit!";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lens {
    pub mode: Mode,
    pub hits: Hits,
    pub switches: Switches,
    pub switching: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pace {
    pub cycle: i32,
    pub recharge: i32,
}

impl Pace {
    pub fn of(stats: &StatContext<'_>) -> Self {
        let recharge = if stats.stats.faction == Faction::Cat { (STAT_COOLDOWN.get_value)(stats) } else { 0 };

        Self { cycle: (STAT_ATK_CYCLE.get_value)(stats), recharge }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    pub attack: f64,
    pub dps: f64,
}

pub fn switch_of(identity: Identity) -> Option<Switch> {
    match identity {
        Identity::CriticalHit => Some(Switch::Critical),
        Identity::SavageBlow => Some(Switch::Savage),
        Identity::WaveAttack | Identity::MiniWave => Some(Switch::Wave),
        Identity::SurgeAttack | Identity::MiniSurge => Some(Switch::Surge),
        Identity::Explosion => Some(Switch::Explosion),
        Identity::Strengthen => Some(Switch::Strengthen),
        Identity::KillAttackUp => Some(Switch::Kills),
        _ => None,
    }
}

pub fn held_switches(stats: &Entity) -> Vec<(Switch, Identity)> {
    REGISTRY
        .iter()
        .filter_map(|ability| switch_of(ability.identity).filter(|_| !(ability.attributes)(stats).is_empty()).map(|switch| (switch, ability.identity)))
        .collect()
}

pub fn switch_key(identity: Identity) -> Option<usize> {
    switch_of(identity).and_then(|switch| Switch::ALL.iter().position(|held| *held == switch))
}

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Base => "Base Stats",
        Mode::Average => "Avg Stats",
        Mode::Maximum => "Max Stats",
    }
}

pub fn mode_named(label: &str) -> Mode {
    MODES.into_iter().find(|mode| mode_label(*mode) == label).unwrap_or_default()
}

pub fn hits_label(hits: Hits) -> &'static str {
    match hits {
        Hits::Only(0) => "1st Hit",
        Hits::Only(1) => "2nd Hit",
        Hits::Only(_) => "3rd Hit",
        Hits::Standing => "Avg Hits",
        Hits::All => "All Hits",
    }
}

fn strikes(stats: &Entity) -> usize {
    stats.strike_frames().len()
}

pub fn hit_choices(stats: &Entity) -> Vec<Hits> {
    match strikes(stats) {
        0 | 1 => vec![Hits::Only(0)],
        count => (0..count).map(Hits::Only).chain([Hits::Standing, Hits::All]).collect(),
    }
}

pub fn hits_of(label: &str) -> Hits {
    [Hits::Only(0), Hits::Only(1), Hits::Only(2), Hits::All].into_iter().find(|hits| hits_label(*hits) == label).unwrap_or_default()
}

pub fn settled(stats: &Entity, hits: Hits) -> Hits {
    match (strikes(stats), hits) {
        (0 | 1, _) => Hits::Only(0),
        (count, Hits::Only(attack)) if attack >= count => Hits::Standing,
        (_, hits) => hits,
    }
}

pub fn shown_hits(stats: &Entity, hits: Hits) -> Hits {
    if strikes(stats) <= 1 { Hits::Only(0) } else { settled(stats, hits) }
}

pub fn picks_hits(stats: &Entity) -> bool {
    strikes(stats) > 1
}

pub fn tag(stats: &Entity, lens: Lens) -> String {
    format!("{}\n{}", hits_label(shown_hits(stats, lens.hits)), mode_label(lens.mode))
}

pub fn rules(global: &GlobalContext<'_>) -> Rules {
    let vault = global.vault;
    let [wave, mini] = vault.vds.skills.wave_frames(&vault.vfs);

    Rules::new(global.param, wave, mini)
}

pub fn rhythm(ctx: &RenderContext<'_>, pace: Pace) -> Option<Rhythm> {
    (pace.cycle > 0 && !ctx.is_conjure_unit).then(|| Rhythm { cycle: pace.cycle, recharge: pace.recharge, rules: rules(&ctx.global) })
}

pub fn plain(ctx: &RenderContext<'_>, pace: Pace, lens: Lens) -> AbilityGroups {
    collect_ability_data_against(ctx, None, rhythm(ctx, pace), lens)
}

pub fn measure(stats: &StatContext<'_>, rules: &Rules, foe: Foe, terms: Terms, lens: Lens) -> Measure {
    let unit = stats.stats;
    let pace = Pace::of(stats);
    let exposure = unit.exposure(pace.cycle, pace.recharge, settled(unit, lens.hits), rules);
    let attack = exposure.damage(unit, stats.strikes(), foe, terms, lens.mode, lens.switches);
    let (period, swings) = match unit.cadence(pace.cycle, pace.recharge) {
        (period, swings) if period > 0 => (period, swings),
        _ => (pace.cycle, 1),
    };
    let dps = if period > 0 { attack * f64::from(swings) * FRAMES_PER_SECOND / f64::from(period) } else { 0.0 };

    Measure { attack, dps }
}

pub fn rate(value: f64) -> String {
    if value > 0.0 && value < 1.0 { trimmed(value) } else { (value.round() as i64).to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only the hits a unit has are offered, and a single hit is shown as the one choice there is.
    #[test]
    fn the_hit_picker_offers_what_the_unit_has() {
        let single = Entity { time_until_attack_1: 10, ..Entity::default() };
        let triple = Entity { time_until_attack_2: 20, time_until_attack_3: 30, ..single.clone() };

        assert_eq!(hit_choices(&single), [Hits::Only(0)]);
        assert_eq!(hit_choices(&triple).iter().map(|hits| hits_label(*hits)).collect::<Vec<_>>(), ["1st Hit", "2nd Hit", "3rd Hit", "Avg Hits", "All Hits"]);
        assert_eq!((settled(&single, Hits::All), shown_hits(&single, Hits::All)), (Hits::Only(0), Hits::Only(0)), "one hit is measured as the 1st Hit it shows");
        assert_eq!(settled(&Entity { time_until_attack_3: 0, ..triple.clone() }, Hits::Only(2)), Hits::Standing, "a picked hit the next unit lacks falls back");
        assert_eq!((mode_named("Max Stats"), hits_of("2nd Hit"), hits_of("Avg Hits")), (Mode::Maximum, Hits::Only(1), Hits::Standing));
        assert_eq!((tag(&triple, Lens::default()), tag(&triple, Lens { mode: Mode::Maximum, hits: Hits::All, ..Lens::default() }), tag(&single, Lens { mode: Mode::Base, hits: Hits::All, ..Lens::default() })), ("Avg Hits\nAvg Stats".to_owned(), "All Hits\nMax Stats".to_owned(), "1st Hit\nBase Stats".to_owned()), "the tag reads in the order the two combos sit");
    }

    // A unit that dies after its attack deals it once a deploy, so its DPS runs on its recharge; one that
    // cannot be deployed again keeps its attack cycle.
    #[test]
    fn a_unit_that_dies_spreads_its_damage_over_its_recharge() {
        let treasure = crate::systems::treasure::Bonus::default();
        let bomber = Entity { attack_1_damage: 3000, time_until_attack_1: 10, attack_1_abilities: 1, standing_range: 100, cooldown: 600, attack_count_total: 1, attack_count_state: 2, ..Entity::default() };
        let stats = StatContext::cat(&bomber, 60, None, &treasure);
        let pace = Pace::of(&stats);
        let measured = measure(&stats, &Rules::default(), Foe::default(), Terms::default(), Lens { mode: Mode::Base, ..Lens::default() });

        assert!(pace.recharge > 0 && pace.recharge != pace.cycle);
        assert!((measured.dps - 3000.0 * 30.0 / f64::from(pace.recharge)).abs() < 1e-9);

        let spirit = Entity { cooldown: 0, ..bomber };
        let spirited = StatContext::cat(&spirit, 60, None, &treasure);
        let measured = measure(&spirited, &Rules::default(), Foe::default(), Terms::default(), Lens { mode: Mode::Base, ..Lens::default() });

        assert!((measured.dps - 3000.0 * 30.0 / f64::from(Pace::of(&spirited).cycle)).abs() < 1e-9);
    }

    // Decimals only show up where a whole number would round a real rate down to nothing.
    #[test]
    fn small_rates_keep_their_decimals() {
        assert_eq!(rate(30.0 / 199.0), "0.15");
        assert_eq!(rate(0.1), "0.1");
        assert_eq!(rate(0.0), "0");
        assert_eq!(rate(1.4), "1");
        assert_eq!(rate(1227.5), "1228");
    }
}
