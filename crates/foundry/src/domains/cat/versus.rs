use codex::combat::{Bearing, Entity, Foe, Identity, Terms};

use crate::systems::combat::abilities::collect_ability_data_against;
use crate::systems::combat::lens::{self, Lens, Pace};
use crate::systems::combat::registry::{get_display_def, AbilityIcon, Against, StatContext};
use crate::systems::combat::{AbilityGroups, AbilityItem, CustomIcon, Mark, RenderContext};

use super::game::stats::get_final_stats;
use super::waiter::unitid;

const UNCHANGED: f64 = 1e-9;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Felt {
    pub attack: Option<(String, Mark)>,
    pub dps: Option<(String, Mark)>,
    pub hitpoints: Option<(String, Mark)>,
}

fn terms(ctx: &RenderContext<'_>, foe: Foe) -> Terms {
    let param = ctx.global.param;

    Terms {
        fruit: ctx.global.treasure.fruit_against(ctx.final_stats, foe).0,
        behemoth_dealt: param.behemoth_slayer_attack_multiplier(),
        behemoth_taken: param.behemoth_slayer_defense_multiplier(),
        sage_dealt: param.sage_slayer_attack_multiplier(),
        sage_taken: param.sage_slayer_defense_multiplier(),
    }
}

pub fn felt(ctx: &RenderContext<'_>, stats: &StatContext<'_>, foe: Foe, lens: Lens) -> Felt {
    let unit = ctx.final_stats;
    let rules = lens::rules(&ctx.global);
    let terms = terms(ctx, foe);
    let measured = lens::measure(stats, &rules, foe, terms, lens);
    let mark = match (foe.is_empty(), foe.has(Identity::TraitMetal)) {
        (true, _) => Mark::None,
        (false, true) if unit.crits() => Mark::Hit,
        (false, true) => Mark::Miss,
        (false, false) => {
            let plain = lens::measure(stats, &rules, Foe::default(), terms, lens);

            if (measured.attack - plain.attack).abs() > UNCHANGED { Mark::Hit } else { Mark::None }
        }
    };
    let share = unit.taken(foe, terms);
    let tougher = !foe.is_empty() && (share - 1.0).abs() > UNCHANGED && share > 0.0;

    Felt {
        attack: Some((lens::rate(measured.attack), mark)),
        dps: Some((lens::rate(measured.dps), mark)),
        hitpoints: tougher.then(|| (((f64::from(unit.hitpoints) / share).round() as i64).to_string(), Mark::Hit)),
    }
}

fn bearing_mark(bearing: Bearing) -> Mark {
    match bearing {
        Bearing::Full => Mark::Hit,
        Bearing::Partial => Mark::Mixed,
        Bearing::Lost => Mark::Miss,
    }
}

fn against(ctx: &RenderContext<'_>, foe: Foe) -> Option<Against> {
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

fn labeled(ctx: &RenderContext<'_>, label: Identity, mark: Mark) -> AbilityItem {
    let (icon_id, custom_icon) = match get_display_def(label).icon {
        AbilityIcon::Standard(id) => (Some(id), CustomIcon::None),
        AbilityIcon::Custom(custom) => (None, custom),
        AbilityIcon::None => (None, CustomIcon::None),
    };

    AbilityItem { identity: label, icon_id, text: ctx.global.names.trait_label(label).to_owned(), custom_icon, border_id: None, mark, dimmed: false, switchable: false }
}

fn owns(stats: &Entity, label: Identity) -> bool {
    match label {
        Identity::TraitMetal => stats.aims_at(label),
        _ => stats.answers(label),
    }
}

fn marked(ctx: &RenderContext<'_>, foe: Foe, spirit: bool, groups: &mut [&mut Vec<AbilityItem>; 6]) -> Vec<Identity> {
    let stats = ctx.final_stats;
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

fn gathered(ctx: &RenderContext<'_>, foe: Foe, spirit: bool, pace: Pace, lens: Lens) -> (AbilityGroups, Vec<Identity>) {
    let (mut traits, mut h1, mut h2, mut b1, mut b2, mut footer) = collect_ability_data_against(ctx, against(ctx, foe), lens::rhythm(ctx, pace), lens);
    let shown = if foe.is_empty() { Vec::new() } else { marked(ctx, foe, spirit, &mut [&mut traits, &mut h1, &mut h2, &mut b1, &mut b2, &mut footer]) };

    ((traits, h1, h2, b1, b2, footer), shown)
}

pub fn abilities(ctx: &RenderContext<'_>, foe: Foe, spirit: bool, pace: Pace, lens: Lens) -> AbilityGroups {
    let ((mut traits, h1, h2, b1, b2, footer), shown) = gathered(ctx, foe, spirit, pace, lens);
    let stats = ctx.final_stats;

    traits.extend(foe.labels().filter(|label| !shown.contains(label)).map(|label| labeled(ctx, label, label_mark(stats, label, spirit))));

    (traits, h1, h2, b1, b2, footer)
}

pub fn palette(ctx: &RenderContext<'_>, foe: Foe, spirit: bool, pace: Pace, lens: Lens) -> AbilityGroups {
    let ((_, h1, h2, b1, b2, footer), _) = gathered(ctx, foe, spirit, pace, lens);
    let stats = ctx.final_stats;
    let picks = Foe::LABELS
        .into_iter()
        .map(|label| {
            let picked = foe.has(label);
            let mark = if picked { label_mark(stats, label, spirit) } else { Mark::None };

            AbilityItem { dimmed: !picked && !owns(stats, label), ..labeled(ctx, label, mark) }
        })
        .collect();

    (picks, h1, h2, b1, b2, footer)
}

pub fn spirit(ctx: &RenderContext<'_>) -> Option<(Entity, Entity)> {
    if ctx.base_stats.conjure_unit_id <= 0 {
        return None;
    }

    let base = unitid(&ctx.global.vault.vfs, ctx.base_stats.conjure_unit_id)?.into_iter().next()?;
    let stats = get_final_stats(&base, ctx.level_curve, ctx.current_level, None, None, ctx.global.treasure);

    Some((base, stats))
}

pub fn spirit_controls(ctx: &RenderContext<'_>) -> bool {
    spirit(ctx).is_some_and(|(_, stats)| stats.controls())
}

pub fn spirit_abilities(ctx: &RenderContext<'_>, foe: Foe) -> AbilityGroups {
    gathered(ctx, foe, false, Pace::default(), Lens::default()).0
}

#[cfg(test)]
mod tests {
    use super::*;

    // The color of a picked trait says what it means for this cat: a crit gets through Metal, though only a cat that
    // targets it is fully green, and without one Metal holds the cat back, only half so when something still works on it.
    #[test]
    fn picked_traits_are_colored_by_what_they_do() {
        let critter = Entity { attack_1_damage: 100, time_until_attack_1: 10, attack_1_abilities: 1, critical_chance: 10, ..Entity::default() };
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

    // In Versus Mode every label is offered, and only the ones the cat has nothing for are dimmed;
    // a crit lands on Metal, but it is not a target.
    #[test]
    fn untargeted_labels_are_dimmed() {
        let critter = Entity { attack_1_damage: 100, attack_1_abilities: 1, critical_chance: 10, trait_red: 1, colossus_slayer: 1, ..Entity::default() };

        assert!(owns(&critter, Identity::TraitRed) && owns(&critter, Identity::TraitColossus));
        assert!(!owns(&critter, Identity::TraitMetal) && !owns(&critter, Identity::TraitAngel));
        assert!(owns(&Entity { trait_metal: 1, ..critter.clone() }, Identity::TraitMetal));
        assert_eq!(Mark::Mixed.tones().map(|(upper, lower)| upper != lower), Some(true));
        assert_eq!(Mark::None.tones(), None);
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
}
