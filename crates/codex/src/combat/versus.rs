use super::abilities::{get_ability, Identity};
use super::entity::{Entity, Faction};

const COLOSSUS_DEALT: i64 = 160;
const COLOSSUS_TAKEN: i64 = 70;
const KILLER_DEALT: i64 = 5;
const WITCH_TAKEN: f64 = 10.0;
const EVA_TAKEN: f64 = 5.0;
const CRITICAL: i64 = 2;
const METAL_FLOOR: i64 = 1;
const WHOLE: i64 = 100;
const MINI_SURGE_SHARE: i64 = 500;

const TARGETED: [Identity; 12] = [
    Identity::TraitRed,
    Identity::TraitFloating,
    Identity::TraitDark,
    Identity::TraitMetal,
    Identity::TraitTraitless,
    Identity::TraitAngel,
    Identity::TraitAlien,
    Identity::TraitZombie,
    Identity::TraitRelic,
    Identity::TraitWitch,
    Identity::TraitEva,
    Identity::TraitAku,
];

const ON_TARGET: [Identity; 12] = [
    Identity::AttackOnly,
    Identity::StrongAgainst,
    Identity::MassiveDamage,
    Identity::InsaneDamage,
    Identity::Resist,
    Identity::InsanelyTough,
    Identity::Dodge,
    Identity::Weaken,
    Identity::Freeze,
    Identity::Slow,
    Identity::Knockback,
    Identity::Curse,
];

const DEALT_ONLY: [Identity; 2] = [Identity::MassiveDamage, Identity::InsaneDamage];

const CONTROLS: [Identity; 5] = [Identity::Freeze, Identity::Slow, Identity::Weaken, Identity::Curse, Identity::Knockback];

const DEALT_AND_TAKEN: [Identity; 6] = [
    Identity::StrongAgainst,
    Identity::WitchKiller,
    Identity::EvaKiller,
    Identity::ColossusSlayer,
    Identity::BehemothSlayer,
    Identity::SageSlayer,
];

const ON_LABEL: [(Identity, Identity); 9] = [
    (Identity::TraitMetal, Identity::CriticalHit),
    (Identity::TraitMetal, Identity::MetalKiller),
    (Identity::TraitZombie, Identity::ZombieKiller),
    (Identity::TraitZombie, Identity::Soulstrike),
    (Identity::TraitWitch, Identity::WitchKiller),
    (Identity::TraitEva, Identity::EvaKiller),
    (Identity::TraitColossus, Identity::ColossusSlayer),
    (Identity::TraitBehemoth, Identity::BehemothSlayer),
    (Identity::TraitSage, Identity::SageSlayer),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Foe(u16);

impl Foe {
    pub const LABELS: [Identity; 15] = [
        Identity::TraitRed,
        Identity::TraitFloating,
        Identity::TraitDark,
        Identity::TraitMetal,
        Identity::TraitAngel,
        Identity::TraitAlien,
        Identity::TraitZombie,
        Identity::TraitRelic,
        Identity::TraitAku,
        Identity::TraitTraitless,
        Identity::TraitWitch,
        Identity::TraitEva,
        Identity::TraitColossus,
        Identity::TraitBehemoth,
        Identity::TraitSage,
    ];

    pub fn from_bits(bits: u16) -> Self {
        Self(bits & ((1 << Self::LABELS.len()) - 1))
    }

    pub fn bits(self) -> u16 {
        self.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn has(self, label: Identity) -> bool {
        Self::LABELS.iter().position(|held| *held == label).is_some_and(|at| self.0 & (1 << at) != 0)
    }

    pub fn with(self, label: Identity) -> Self {
        Self::LABELS.iter().position(|held| *held == label).map_or(self, |at| Self(self.0 | (1 << at)))
    }

    pub fn of(stats: &Entity) -> Self {
        Self::LABELS.into_iter().filter(|label| carried(stats, *label) != 0).fold(Self::default(), Self::with)
    }

    pub fn labels(self) -> impl Iterator<Item = Identity> {
        Self::LABELS.into_iter().filter(move |label| self.has(*label))
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn without(self, label: Identity) -> Self {
        Self::LABELS.iter().position(|held| *held == label).map_or(self, |at| Self(self.0 & !(1 << at)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bearing {
    Full,
    Partial,
    Lost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Terms {
    pub fruit: i32,
    pub behemoth_dealt: i32,
    pub behemoth_taken: i32,
    pub sage_dealt: i32,
    pub sage_taken: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrier {
    Direct,
    Wave,
    MiniWave(i32),
    Surge,
    MiniSurge,
    Explosion(i32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Roll {
    pub critical: bool,
    pub savage: bool,
    pub strengthened: bool,
    pub rampant: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortfall {
    Metal,
    Sage,
}

fn flag(stats: &Entity, label: Identity) -> i32 {
    match label {
        Identity::TraitRed => stats.trait_red,
        Identity::TraitFloating => stats.trait_floating,
        Identity::TraitDark => stats.trait_dark,
        Identity::TraitMetal => stats.trait_metal,
        Identity::TraitTraitless => stats.trait_traitless,
        Identity::TraitAngel => stats.trait_angel,
        Identity::TraitAlien => stats.trait_alien,
        Identity::TraitZombie => stats.trait_zombie,
        Identity::TraitRelic => stats.trait_relic,
        Identity::TraitWitch => stats.trait_witch,
        Identity::TraitEva => stats.trait_eva,
        Identity::TraitAku => stats.trait_aku,
        _ => 0,
    }
}

fn carried(stats: &Entity, label: Identity) -> i32 {
    match label {
        Identity::TraitColossus => stats.trait_colossus,
        Identity::TraitBehemoth => stats.trait_behemoth,
        Identity::TraitSage => stats.trait_sage,
        _ => flag(stats, label),
    }
}

pub fn resisted(value: i32, percent: i32) -> i32 {
    (i64::from(value) * i64::from(100 - percent) / 100) as i32
}

impl Entity {
    pub fn aims_at(&self, label: Identity) -> bool {
        flag(self, label) != 0
    }

    pub fn targets(&self, foe: Foe) -> bool {
        TARGETED.iter().any(|label| foe.has(*label) && self.aims_at(*label))
    }

    pub fn crits(&self) -> bool {
        let hits = [(self.attack_1_damage, self.attack_1_abilities), (self.attack_2_damage, self.attack_2_abilities), (self.attack_3_damage, self.attack_3_abilities)];

        self.critical_chance > 0 && hits.iter().any(|(attack, able)| *attack != 0 && *able != 0)
    }

    fn holds(&self, ability: Identity) -> bool {
        get_ability(ability).is_some_and(|held| !(held.attributes)(self).is_empty())
    }

    pub fn answers(&self, label: Identity) -> bool {
        self.aims_at(label) || ON_LABEL.iter().any(|(held, ability)| *held == label && self.holds(*ability))
    }

    pub fn controls(&self) -> bool {
        CONTROLS.iter().any(|ability| self.holds(*ability))
    }

    pub fn answered(&self) -> Foe {
        Foe::LABELS.into_iter().filter(|label| self.answers(*label)).fold(Foe::default(), Foe::with)
    }

    pub fn touched(&self, foe: Foe) -> Vec<(Identity, Bearing)> {
        let blunted = foe.has(Identity::TraitMetal) && !self.crits();
        let soothed = foe.has(Identity::TraitSage) && self.sage_slayer == 0;
        let bearing = |ability: Identity| match (blunted, DEALT_ONLY.contains(&ability), DEALT_AND_TAKEN.contains(&ability)) {
            (true, true, _) => Bearing::Lost,
            (true, _, true) => Bearing::Partial,
            _ if soothed && CONTROLS.contains(&ability) => Bearing::Lost,
            _ => Bearing::Full,
        };
        let aimed = match (self.targets(foe), soothed) {
            (true, _) => &ON_TARGET[..],
            (false, true) => &CONTROLS[..],
            (false, false) => &[],
        };
        let labeled = ON_LABEL.iter().filter(|(label, _)| foe.has(*label)).map(|(_, ability)| *ability);

        aimed.iter().copied().chain(labeled).filter(|ability| self.holds(*ability)).map(|ability| (ability, bearing(ability))).collect()
    }

    pub fn shortfall(&self, foe: Foe, ability: Identity) -> Option<Shortfall> {
        let blunted = foe.has(Identity::TraitMetal) && !self.crits();
        let soothed = foe.has(Identity::TraitSage) && self.sage_slayer == 0;

        match ability {
            _ if blunted && (DEALT_ONLY.contains(&ability) || DEALT_AND_TAKEN.contains(&ability)) => Some(Shortfall::Metal),
            _ if soothed && CONTROLS.contains(&ability) => Some(Shortfall::Sage),
            _ => None,
        }
    }

    pub fn landing(&self, foe: Foe, terms: Terms, attack: i32, carrier: Carrier, roll: Roll) -> i64 {
        let fruit = i64::from(terms.fruit);
        let scale = match carrier {
            Carrier::Explosion(percent) => i64::from(percent),
            _ => WHOLE,
        };
        let raw = i64::from(attack) * scale;
        let mut damage = match carrier {
            Carrier::MiniSurge => raw / MINI_SURGE_SHARE,
            Carrier::MiniWave(percent) => raw / WHOLE * i64::from(percent) / WHOLE,
            _ => raw / WHOLE,
        };

        if roll.critical {
            damage *= CRITICAL;
        }

        if roll.strengthened && self.strengthen_threshold > 0 {
            damage = damage * (WHOLE + i64::from(self.strengthen_boost)) / WHOLE;
        }

        if roll.rampant && self.faction == Faction::Cat && self.kill_attack_up > 0 {
            damage = damage * (WHOLE + i64::from(self.kill_attack_up)) / WHOLE;
        }

        if roll.savage {
            damage = damage * (WHOLE + i64::from(self.savage_blow_boost)) / WHOLE;
        }

        if self.targets(foe) {
            if self.strong_against != 0 {
                damage = damage * (fruit + i64::from(self.effect_up(Identity::StrongAgainst)) * 10 + 1500) / 1000;
            }

            if self.massive_damage != 0 {
                damage = damage * (i64::from(self.effect_up(Identity::MassiveDamage)) * 3 + fruit + 900) / 300;
            }

            if self.insane_damage != 0 {
                damage = damage * (fruit + 1500) / 300;
            }
        }

        if self.witch_killer != 0 && foe.has(Identity::TraitWitch) {
            damage *= KILLER_DEALT;
        }

        if self.eva_killer != 0 && foe.has(Identity::TraitEva) {
            damage *= KILLER_DEALT;
        }

        if self.colossus_slayer != 0 && foe.has(Identity::TraitColossus) {
            damage = damage * COLOSSUS_DEALT / 100;
        }

        if self.behemoth_slayer != 0 && foe.has(Identity::TraitBehemoth) {
            damage = damage * i64::from(terms.behemoth_dealt) / 1000;
        }

        if self.sage_slayer != 0 && foe.has(Identity::TraitSage) {
            damage = damage * i64::from(terms.sage_dealt) / 1000;
        }

        if foe.has(Identity::TraitMetal) && !roll.critical && damage > 0 {
            damage = METAL_FLOOR;
        }

        damage.max(0)
    }

    pub fn taken(&self, foe: Foe, terms: Terms) -> f64 {
        let fruit = f64::from(terms.fruit);
        let eased = |identity: Identity| f64::from(100 - self.effect_up(identity)) / 100.0;
        let mut share = 1.0;

        if self.targets(foe) {
            if self.strong_against != 0 {
                share *= (1500.0 - fruit) / 3000.0 * eased(Identity::StrongAgainst);
            }

            if self.resist != 0 {
                share *= (1500.0 - fruit) / 6000.0 * eased(Identity::Resist);
            }

            if self.insanely_tough != 0 {
                share *= (2100.0 - fruit) / 12600.0;
            }
        }

        if self.witch_killer != 0 && foe.has(Identity::TraitWitch) {
            share /= WITCH_TAKEN;
        }

        if self.eva_killer != 0 && foe.has(Identity::TraitEva) {
            share /= EVA_TAKEN;
        }

        if self.colossus_slayer != 0 && foe.has(Identity::TraitColossus) {
            share *= COLOSSUS_TAKEN as f64 / 100.0;
        }

        if self.behemoth_slayer != 0 && foe.has(Identity::TraitBehemoth) {
            share *= f64::from(terms.behemoth_taken) / 1000.0;
        }

        if self.sage_slayer != 0 && foe.has(Identity::TraitSage) {
            share *= f64::from(terms.sage_taken) / 1000.0;
        }

        share
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMS: Terms = Terms { fruit: 300, behemoth_dealt: 2500, behemoth_taken: 400, sage_dealt: 1200, sage_taken: 500 };

    fn foe(labels: &[Identity]) -> Foe {
        labels.iter().fold(Foe::default(), |foe, label| foe.with(*label))
    }

    // The set travels in a button as its bits, so a label's place in the list can never change.
    #[test]
    fn a_foe_survives_its_bits() {
        let held = foe(&[Identity::TraitRed, Identity::TraitMetal, Identity::TraitSage]);

        assert_eq!(Foe::from_bits(held.bits()), held);
        assert_eq!(held.labels().collect::<Vec<_>>(), [Identity::TraitRed, Identity::TraitMetal, Identity::TraitSage]);
        assert_eq!((Foe::LABELS[0], Foe::LABELS[3], Foe::LABELS[14]), (Identity::TraitRed, Identity::TraitMetal, Identity::TraitSage));
        assert_eq!(Foe::from_bits(u16::MAX).labels().count(), Foe::LABELS.len());
        assert_eq!(held.without(Identity::TraitMetal).union(foe(&[Identity::TraitAngel])).labels().collect::<Vec<_>>(), [Identity::TraitRed, Identity::TraitAngel, Identity::TraitSage]);
        assert!(Foe::default().with(Identity::CriticalHit).is_empty());
    }

    // A trait multiplier needs the unit to target a trait the enemy has; one shared trait is enough for the whole set.
    #[test]
    fn trait_damage_needs_a_shared_trait() {
        let unit = Entity { attack_1_damage: 1000, trait_red: 1, massive_damage: 1, ..Entity::default() };

        assert_eq!(unit.landing(foe(&[Identity::TraitRed]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 4000);
        assert_eq!(unit.landing(foe(&[Identity::TraitRed, Identity::TraitAngel]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 4000);
        assert_eq!(unit.landing(foe(&[Identity::TraitAngel]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 1000);
        assert_eq!(unit.landing(Foe::default(), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 1000);
        assert_eq!(Entity { strong_against: 1, ..unit }.landing(foe(&[Identity::TraitRed]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 7200, "strong first, then massive, each rounded down on its own");
    }

    // Against Metal a landing that is not a critical hit deals 1, whatever came before; a critical one is doubled first and then multiplied.
    #[test]
    fn metal_takes_one_unless_the_hit_is_critical() {
        let unit = Entity { attack_1_damage: 1000, attack_1_abilities: 1, trait_metal: 1, massive_damage: 1, critical_chance: 30, ..Entity::default() };
        let metal = foe(&[Identity::TraitMetal]);
        let land = |carrier: Carrier, critical: bool, savage: bool| unit.landing(metal, TERMS, 1000, carrier, Roll { critical, savage, ..Roll::default() });

        assert_eq!((land(Carrier::Direct, false, false), land(Carrier::Direct, true, false)), (1, 8000));
        assert_eq!(land(Carrier::Direct, false, true), 1, "savage alone is still 1 against Metal");
        assert!(unit.crits());
        assert!(!Entity { critical_chance: 0, ..unit.clone() }.crits());
        assert!(!Entity { attack_1_abilities: 0, ..unit.clone() }.crits(), "a chance on a hit that carries no abilities never fires");
    }

    // A carrier is scaled before the crit doubles it and savage multiplies it, each step rounded down.
    #[test]
    fn carriers_scale_before_the_rolls() {
        let unit = Entity { savage_blow_boost: 200, ..Entity::default() };
        let land = |carrier: Carrier, critical: bool, savage: bool| unit.landing(Foe::default(), TERMS, 1001, carrier, Roll { critical, savage, ..Roll::default() });

        assert_eq!(land(Carrier::Direct, true, true), 1001 * 2 * 3);
        assert_eq!(land(Carrier::MiniWave(20), false, false), 200);
        assert_eq!(land(Carrier::MiniSurge, true, false), 400);
        assert_eq!(land(Carrier::Explosion(70), false, true), 700 * 3);
        assert_eq!(land(Carrier::Surge, false, false), land(Carrier::Wave, false, false));
    }

    // Strengthen and the kill boost come right after the crit doubles a landing and before savage,
    // and only a cat carries the kill boost.
    #[test]
    fn boosts_land_between_the_crit_and_savage() {
        let unit = Entity { strengthen_threshold: 50, strengthen_boost: 100, kill_attack_up: 50, savage_blow_boost: 200, ..Entity::default() };
        let land = |unit: &Entity, boosted: bool| unit.landing(Foe::default(), TERMS, 1000, Carrier::Direct, Roll { critical: true, savage: true, strengthened: boosted, rampant: boosted });

        assert_eq!(land(&unit, true), 1000 * 2 * 2 * 3 / 2 * 3);
        assert_eq!(land(&unit, false), 1000 * 2 * 3);
        assert_eq!(land(&Entity { faction: Faction::Enemy, ..unit.clone() }, true), 1000 * 2 * 2 * 3, "an enemy has no kill boost");
    }

    // Only Metal without a crit and a Sage without a slayer hold an ability back.
    #[test]
    fn a_shortfall_names_its_cause() {
        let unit = Entity { attack_1_damage: 1000, attack_1_abilities: 1, trait_metal: 1, massive_damage: 1, freeze_chance: 30, freeze_duration: 30, ..Entity::default() };

        assert_eq!(unit.shortfall(foe(&[Identity::TraitMetal]), Identity::MassiveDamage), Some(Shortfall::Metal));
        assert_eq!(unit.shortfall(foe(&[Identity::TraitSage]), Identity::Freeze), Some(Shortfall::Sage));
        assert_eq!(Entity { critical_chance: 10, ..unit.clone() }.shortfall(foe(&[Identity::TraitMetal]), Identity::MassiveDamage), None);
        assert_eq!(unit.shortfall(foe(&[Identity::TraitRed]), Identity::Freeze), None);
    }

    // The killers and slayers read the enemy's label alone, with no trait of the unit's own needed.
    #[test]
    fn killers_need_only_the_label() {
        let unit = Entity { attack_1_damage: 1000, witch_killer: 1, colossus_slayer: 1, behemoth_slayer: 1, sage_slayer: 1, ..Entity::default() };

        assert_eq!(unit.landing(foe(&[Identity::TraitWitch]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 5000);
        assert_eq!(unit.landing(foe(&[Identity::TraitColossus]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 1600);
        assert_eq!(unit.landing(foe(&[Identity::TraitBehemoth]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 2500);
        assert_eq!(unit.landing(foe(&[Identity::TraitSage]), TERMS, 1000, Carrier::Direct, Roll { critical: false, savage: false, ..Roll::default() }), 1200);
        assert_eq!(unit.taken(foe(&[Identity::TraitWitch]), TERMS), 0.1);
        assert_eq!(unit.taken(foe(&[Identity::TraitColossus]), TERMS), 0.7);
        assert_eq!(unit.touched(foe(&[Identity::TraitWitch])), [(Identity::WitchKiller, Bearing::Full)]);
        assert!(unit.answers(Identity::TraitColossus) && unit.answers(Identity::TraitWitch));
        assert!(!unit.answers(Identity::TraitRed) && !unit.answers(Identity::TraitEva));
        assert!(Entity { trait_red: 1, ..Entity::default() }.answers(Identity::TraitRed));
        assert_eq!(Entity { trait_red: 1, ..unit.clone() }.answered().labels().collect::<Vec<_>>(), [Identity::TraitRed, Identity::TraitWitch, Identity::TraitColossus, Identity::TraitBehemoth, Identity::TraitSage]);
    }

    // Against Metal without a crit every hit deals 1, so a boost that only adds damage does nothing,
    // one that also cuts damage taken still does half its job, and a crit brings both back.
    #[test]
    fn metal_without_a_crit_blunts_damage_boosts() {
        let unit = Entity { attack_1_damage: 1000, attack_1_abilities: 1, trait_metal: 1, massive_damage: 1, strong_against: 1, resist: 1, freeze_chance: 30, freeze_duration: 30, ..Entity::default() };
        let metal = foe(&[Identity::TraitMetal]);
        let bearing = |unit: &Entity, ability: Identity| unit.touched(metal).into_iter().find(|(held, _)| *held == ability).map(|(_, bearing)| bearing);

        assert_eq!(bearing(&unit, Identity::MassiveDamage), Some(Bearing::Lost));
        assert_eq!(bearing(&unit, Identity::StrongAgainst), Some(Bearing::Partial));
        assert_eq!(bearing(&unit, Identity::Resist), Some(Bearing::Full));
        assert_eq!(bearing(&unit, Identity::Freeze), Some(Bearing::Full));
        assert_eq!(bearing(&unit, Identity::InsaneDamage), None, "only what the unit has is listed");

        let critter = Entity { critical_chance: 5, ..unit.clone() };

        assert_eq!(bearing(&critter, Identity::MassiveDamage), Some(Bearing::Full));
        assert_eq!(bearing(&critter, Identity::StrongAgainst), Some(Bearing::Full));
        assert_eq!(unit.touched(foe(&[Identity::TraitRed])), [], "a unit that targets only Metal has nothing against Red");
        assert_eq!(critter.landing(metal, TERMS, 1000, Carrier::Direct, Roll { critical: true, savage: false, ..Roll::default() }), 1000 * 2 * 1800 / 1000 * 1200 / 300, "a crit keeps Strong and Massive, Metal fruit included");
    }

    // A Sage shrugs off most of every status a unit without Sage Slayer lands, so those count against the unit,
    // and they are listed even when nothing else about the Sage is targeted.
    #[test]
    fn sage_weakens_crowd_control_without_a_slayer() {
        let unit = Entity { trait_red: 1, freeze_chance: 30, freeze_duration: 60, knockback_chance: 20, ..Entity::default() };
        let sage = foe(&[Identity::TraitRed, Identity::TraitSage]);
        let bearing = |unit: &Entity, ability: Identity| unit.touched(sage).into_iter().find(|(held, _)| *held == ability).map(|(_, bearing)| bearing);

        assert!(unit.controls());
        assert!(!Entity { trait_red: 1, ..Entity::default() }.controls());
        assert_eq!(bearing(&unit, Identity::Freeze), Some(Bearing::Lost));
        assert_eq!(bearing(&unit, Identity::Knockback), Some(Bearing::Lost));
        assert_eq!(unit.touched(foe(&[Identity::TraitSage])), [(Identity::Freeze, Bearing::Lost), (Identity::Knockback, Bearing::Lost)]);
        assert_eq!(Entity { sage_slayer: 1, ..unit.clone() }.touched(foe(&[Identity::TraitSage])), [(Identity::SageSlayer, Bearing::Full)]);
        assert_eq!(bearing(&Entity { sage_slayer: 1, ..unit.clone() }, Identity::Freeze), Some(Bearing::Full));
        assert_eq!(unit.touched(foe(&[Identity::TraitRed])), [(Identity::Freeze, Bearing::Full), (Identity::Knockback, Bearing::Full)]);
        assert_eq!(resisted(149, 50), 74);
    }

    // An enemy's own labels, subtraits included, are what it brings to a fight.
    #[test]
    fn an_enemy_carries_its_labels() {
        let enemy = Entity { trait_dark: 1, trait_relic: 1, trait_sage: 1, trait_starred_alien: 1, ..Entity::default() };

        assert_eq!(Foe::of(&enemy).labels().collect::<Vec<_>>(), [Identity::TraitDark, Identity::TraitRelic, Identity::TraitSage]);
    }

    #[test]
    fn damage_taken_follows_the_shared_trait() {
        let unit = Entity { trait_red: 1, resist: 1, ..Entity::default() };

        assert_eq!(unit.taken(foe(&[Identity::TraitRed]), TERMS), 0.2);
        assert_eq!(unit.taken(foe(&[Identity::TraitAngel]), TERMS), 1.0);
        assert!((Entity { insanely_tough: 1, resist: 0, ..unit }.taken(foe(&[Identity::TraitRed]), Terms::default()) - 1.0 / 6.0).abs() < 1e-12);
    }
}
