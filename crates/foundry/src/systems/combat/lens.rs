use codex::combat::{Entity, Faction, Foe, Hits, Mode, Rules, Terms};

use crate::common::context::GlobalContext;
use crate::common::decimal::trimmed;
use crate::systems::combat::abilities::collect_ability_data_against;
use crate::systems::combat::registry::{Rhythm, StatContext, STAT_ATK_CYCLE, STAT_COOLDOWN};
use crate::systems::combat::{AbilityGroups, RenderContext};

const FRAMES_PER_SECOND: f64 = 30.0;

pub const MODES: [Mode; 3] = [Mode::Minimum, Mode::Average, Mode::Maximum];
pub const ONE_HIT: &str = "This unit only has one hit!";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lens {
    pub mode: Mode,
    pub hits: Hits,
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

pub fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Minimum => "Minimum",
        Mode::Average => "Average",
        Mode::Maximum => "Maximum",
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
    let attack = exposure.damage(unit, stats.strikes(), foe, terms, lens.mode);
    let dps = if pace.cycle > 0 { attack * FRAMES_PER_SECOND / f64::from(pace.cycle) } else { 0.0 };

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
        assert_eq!((mode_named("Maximum"), hits_of("2nd Hit"), hits_of("Avg Hits")), (Mode::Maximum, Hits::Only(1), Hits::Standing));
        assert_eq!((tag(&triple, Lens::default()), tag(&triple, Lens { mode: Mode::Maximum, hits: Hits::All }), tag(&single, Lens { mode: Mode::Minimum, hits: Hits::All })), ("Avg Hits\nAverage".to_owned(), "All Hits\nMaximum".to_owned(), "1st Hit\nMinimum".to_owned()), "the tag reads in the order the two combos sit");
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
