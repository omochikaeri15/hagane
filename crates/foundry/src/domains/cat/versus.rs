use codex::combat::{Bearing, Entity, Foe, Identity, Terms};

use crate::common::decimal::trimmed;
use crate::systems::combat::registry::{get_display_def, AbilityIcon, Against};
use crate::systems::combat::{AbilityItem, CustomIcon, Mark, RenderContext};

const FRAMES_PER_SECOND: f64 = 30.0;
const UNCHANGED: f64 = 1e-9;

#[derive(Default)]
pub(super) struct Felt {
    pub(super) attack: Option<(String, Mark)>,
    pub(super) dps: Option<(String, Mark)>,
    pub(super) hitpoints: Option<(String, Mark)>,
}

fn rate(per_second: f64) -> String {
    if per_second > 0.0 && per_second < 1.0 { trimmed(per_second) } else { (per_second.round() as i64).to_string() }
}

fn terms(ctx: &RenderContext<'_>, foe: Foe) -> Terms {
    let param = ctx.global.param;

    Terms {
        fruit: ctx.global.treasure.fruit_against(ctx.final_stats, foe).0,
        behemoth_dealt: param.behemoth_slayer_attack_multiplier,
        behemoth_taken: param.behemoth_slayer_defense_multiplier,
        sage_dealt: param.sage_slayer_attack_multiplier,
        sage_taken: param.sage_slayer_defense_multiplier,
    }
}

pub(super) fn felt(ctx: &RenderContext<'_>, cycle: i32, foe: Foe) -> Felt {
    if foe.is_empty() {
        return Felt::default();
    }

    let stats = ctx.final_stats;
    let terms = terms(ctx, foe);
    let plain = i64::from(stats.attack_1_damage) + i64::from(stats.attack_2_damage) + i64::from(stats.attack_3_damage);
    let strike = stats.strike(foe, terms);
    let mark = match strike.pierced {
        Some(false) => Some(Mark::Miss),
        Some(true) => Some(Mark::Hit),
        None => (strike.landed != plain).then_some(Mark::Hit),
    };
    let per_second = if cycle > 0 { strike.expected * FRAMES_PER_SECOND / f64::from(cycle) } else { 0.0 };
    let share = stats.taken(foe, terms);
    let tougher = (share - 1.0).abs() > UNCHANGED && share > 0.0;

    Felt {
        attack: mark.map(|mark| (strike.landed.to_string(), mark)),
        dps: mark.map(|mark| (rate(per_second), mark)),
        hitpoints: tougher.then(|| (((f64::from(stats.hitpoints) / share).round() as i64).to_string(), Mark::Hit)),
    }
}

fn bearing_mark(bearing: Bearing) -> Mark {
    match bearing {
        Bearing::Full => Mark::Hit,
        Bearing::Partial => Mark::Mixed,
        Bearing::Lost => Mark::Miss,
    }
}

pub(super) fn against(ctx: &RenderContext<'_>, foe: Foe) -> Option<Against> {
    let stats = ctx.final_stats;
    let treasure = ctx.global.treasure;
    let sage = foe.has(Identity::TraitSage) && stats.sage_slayer == 0;

    (stats.targets(foe) || sage).then(|| Against { fruit: treasure.fruit_against(stats, foe), push: treasure.push_fruit(foe), sage })
}

fn metal_mark(stats: &Entity) -> Mark {
    let kept = || stats.touched(Foe::default().with(Identity::TraitMetal)).iter().any(|(_, bearing)| *bearing != Bearing::Lost);

    match (stats.crits(), stats.aims_at(Identity::TraitMetal)) {
        (true, true) => Mark::Hit,
        (true, false) => Mark::Glancing,
        (false, _) if kept() => Mark::Mixed,
        (false, _) => Mark::Miss,
    }
}

fn sage_mark(stats: &Entity, spirit: bool) -> Mark {
    match (stats.sage_slayer != 0, stats.controls() || spirit) {
        (true, true) => Mark::Hit,
        (true, false) => Mark::Glancing,
        (false, true) => Mark::Miss,
        (false, false) => Mark::Inert,
    }
}

fn label_mark(stats: &Entity, label: Identity, spirit: bool) -> Mark {
    match label {
        Identity::TraitMetal => metal_mark(stats),
        Identity::TraitSage => sage_mark(stats, spirit),
        _ if stats.answers(label) => Mark::Hit,
        _ => Mark::Inert,
    }
}

fn labeled(label: Identity, mark: Mark) -> AbilityItem {
    let (icon_id, custom_icon) = match get_display_def(label).icon {
        AbilityIcon::Standard(id) => (Some(id), CustomIcon::None),
        AbilityIcon::Custom(custom) => (None, custom),
        AbilityIcon::None => (None, CustomIcon::None),
    };

    AbilityItem { identity: label, icon_id, text: String::new(), custom_icon, border_id: None, mark, added: true }
}

fn marked(stats: &Entity, foe: Foe, spirit: bool, groups: &mut [&mut Vec<AbilityItem>; 6]) -> Vec<Identity> {
    let touched = stats.touched(foe);
    let mut shown = Vec::new();

    for item in groups.iter_mut().flat_map(|group| group.iter_mut()) {
        if foe.has(item.identity) {
            item.mark = label_mark(stats, item.identity, spirit);
            shown.push(item.identity);
        } else if let Some((_, bearing)) = touched.iter().find(|(ability, _)| *ability == item.identity) {
            item.mark = bearing_mark(*bearing);
        }
    }

    shown
}

pub(super) fn mark(ctx: &RenderContext<'_>, foe: Foe, mut groups: [&mut Vec<AbilityItem>; 6]) {
    if !foe.is_empty() {
        marked(ctx.final_stats, foe, false, &mut groups);
    }
}

pub(super) fn tag(ctx: &RenderContext<'_>, foe: Foe, spirit: bool, mut groups: [&mut Vec<AbilityItem>; 6]) {
    if foe.is_empty() {
        return;
    }

    let stats = ctx.final_stats;
    let shown = marked(stats, foe, spirit, &mut groups);
    let absent: Vec<AbilityItem> = foe.labels().filter(|label| !shown.contains(label)).map(|label| labeled(label, label_mark(stats, label, spirit))).collect();

    groups[0].extend(absent);
}

#[cfg(test)]
mod tests {
    use super::*;

    // The color of a picked trait says what it means for this cat: a crit gets through Metal, though only a cat that
    // targets it is fully green, and without one Metal holds the cat back, only half so when something still works on it.
    #[test]
    fn picked_traits_are_colored_by_what_they_do() {
        let critter = Entity { attack_1_damage: 100, attack_1_abilities: 1, critical_chance: 10, ..Entity::default() };
        let aimed = Entity { trait_metal: 1, trait_red: 1, ..critter.clone() };
        let blunt = Entity { critical_chance: 0, massive_damage: 1, ..aimed.clone() };
        let guarded = Entity { strong_against: 1, ..blunt.clone() };
        let slayer = Entity { colossus_slayer: 1, ..Entity::default() };

        assert_eq!(label_mark(&aimed, Identity::TraitMetal, false), Mark::Hit);
        assert_eq!(label_mark(&blunt, Identity::TraitMetal, false), Mark::Miss, "Massive alone does nothing without a crit");
        assert_eq!(label_mark(&guarded, Identity::TraitMetal, false), Mark::Mixed, "Strong still cuts the damage it takes");
        assert_eq!(label_mark(&critter, Identity::TraitMetal, false), Mark::Glancing, "its crits get through, but it doesn't target Metal");
        assert_eq!(label_mark(&Entity { critical_chance: 0, metal_killer_percent: 10, ..critter.clone() }, Identity::TraitMetal, false), Mark::Mixed);
        assert_eq!(label_mark(&Entity::default(), Identity::TraitMetal, false), Mark::Miss);
        assert_eq!(label_mark(&aimed, Identity::TraitRed, false), Mark::Hit);
        assert_eq!(label_mark(&aimed, Identity::TraitAngel, false), Mark::Inert);
        assert_eq!(label_mark(&slayer, Identity::TraitColossus, false), Mark::Hit);
        assert_eq!(label_mark(&slayer, Identity::TraitSage, false), Mark::Inert);
    }

    // A Sage only matters to a cat through its crowd control, which it shrugs off unless the cat is a Sage Slayer.
    #[test]
    fn sage_is_colored_by_crowd_control() {
        let freezer = Entity { freeze_chance: 30, freeze_duration: 60, ..Entity::default() };
        let slayer = Entity { sage_slayer: 1, ..Entity::default() };

        assert_eq!(label_mark(&freezer, Identity::TraitSage, false), Mark::Miss);
        assert_eq!(label_mark(&Entity { sage_slayer: 1, ..freezer.clone() }, Identity::TraitSage, false), Mark::Hit);
        assert_eq!(label_mark(&slayer, Identity::TraitSage, false), Mark::Glancing);
        assert_eq!(label_mark(&Entity::default(), Identity::TraitSage, false), Mark::Inert);
        assert_eq!(label_mark(&Entity::default(), Identity::TraitSage, true), Mark::Miss, "a spirit's crowd control counts as the cat's");
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
