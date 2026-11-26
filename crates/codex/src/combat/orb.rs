use serde::{Deserialize, Serialize};

use super::abilities::Identity;
use super::entity::Entity;
use super::output::{Mode, Switch, Switches};
use super::versus::Foe;

pub const FIRST_FORM: usize = 2;

const LOCKED: i32 = 1;
const UNLOCK_LEVEL: i32 = 60;
const RECHARGE_FLOOR: i32 = 60;

pub const ATTACK_UP: i32 = 0;
pub const DEFENSE_UP: i32 = 1;
pub const STRONG_UP: i32 = 2;
pub const MASSIVE_UP: i32 = 3;
pub const TOUGH_UP: i32 = 4;
pub const DEATH_SURGE: i32 = 5;
pub const RESIST_WAVE: i32 = 6;
pub const CASH_BACK: i32 = 7;
pub const RESIST_KNOCKBACK: i32 = 8;
pub const BOOST_LEGEND: i32 = 9;
pub const COLOSSUS: i32 = 10;
pub const CANNON_CHARGE: i32 = 11;
pub const RESIST_TOXIC: i32 = 12;
pub const DODGE: i32 = 13;
pub const RESIST_SLOW: i32 = 14;
pub const RESIST_CURSE: i32 = 15;
pub const BOOST_UNCANNY: i32 = 16;
pub const COUNTER_SURGE: i32 = 17;
pub const BERSERKER: i32 = 18;
pub const COOLDOWN: i32 = 19;
pub const RESIST_FREEZE: i32 = 20;
pub const RESIST_WEAKEN: i32 = 21;
pub const COST_DOWN: i32 = 22;
pub const RESIST_SURGE: i32 = 23;
pub const FIRST_BOUNTY: i32 = 24;
pub const RESIST_EXPLOSION: i32 = 25;

pub const TRAITS: [Identity; 12] = [
    Identity::TraitRed,
    Identity::TraitFloating,
    Identity::TraitDark,
    Identity::TraitMetal,
    Identity::TraitAngel,
    Identity::TraitAlien,
    Identity::TraitZombie,
    Identity::TraitRelic,
    Identity::TraitTraitless,
    Identity::TraitWitch,
    Identity::TraitEva,
    Identity::TraitAku,
];

pub const RESISTS: [(i32, Identity); 8] = [
    (RESIST_WEAKEN, Identity::ResistWeaken),
    (RESIST_FREEZE, Identity::ResistFreeze),
    (RESIST_SLOW, Identity::ResistSlow),
    (RESIST_KNOCKBACK, Identity::ResistKnockback),
    (RESIST_WAVE, Identity::ResistWave),
    (RESIST_CURSE, Identity::ResistCurse),
    (RESIST_TOXIC, Identity::ResistToxic),
    (RESIST_SURGE, Identity::ResistSurge),
];

pub const TOGGLED: [i32; 4] = [COST_DOWN, CASH_BACK, COOLDOWN, BERSERKER];

const SLOT_SWITCHES: [Switch; 2] = [Switch::Orb1, Switch::Orb2];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Terms {
    pub cost_cut: i32,
    pub refund: i32,
    pub recharge_cut: i32,
    pub halved: bool,
    pub berserk: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worn {
    pub slot: usize,
    pub id: u32,
    pub content: i32,
    pub mask: u32,
    pub grade: i32,
    pub values: Vec<i32>,
}

impl Worn {
    pub fn value(&self, param: usize) -> i32 {
        self.values.get(param).copied().unwrap_or(0)
    }

    pub fn strikes(&self, foe: Foe) -> bool {
        bits(foe) & self.mask != 0
    }

    pub fn traits(&self) -> impl Iterator<Item = Identity> + '_ {
        TRAITS.into_iter().enumerate().filter(|(bit, _)| self.mask & (1 << bit) != 0).map(|(_, label)| label)
    }

    pub fn switch(&self) -> Option<Switch> {
        TOGGLED.contains(&self.content).then(|| SLOT_SWITCHES.get(self.slot).copied()).flatten()
    }

    pub fn toggled(&self, switches: Switches, mode: Mode) -> bool {
        let fallback = match self.content {
            BERSERKER => mode == Mode::Maximum,
            _ => mode != Mode::Base,
        };

        self.switch().is_some_and(|switch| switches.held(switch).unwrap_or(fallback))
    }
}

pub fn bits(foe: Foe) -> u32 {
    TRAITS.into_iter().enumerate().filter(|(_, label)| foe.has(*label)).fold(0, |mask, (bit, _)| mask | (1 << bit))
}

pub fn unlocked(condition: i32, level: i32) -> bool {
    condition != LOCKED || level >= UNLOCK_LEVEL
}

impl Entity {
    pub fn orb_sum(&self, content: i32, param: usize) -> i32 {
        self.orbs.iter().filter(|worn| worn.content == content).fold(0, |sum, worn| sum.wrapping_add(worn.value(param)))
    }

    pub fn orb_max(&self, content: i32, param: usize) -> i32 {
        let mut best = -1;
        let mut value = 0;

        for worn in self.orbs.iter().filter(|worn| worn.content == content) {
            if worn.grade > best {
                value = worn.value(param);
                best = worn.grade;
            }
        }

        value
    }

    pub fn orb_versus(&self, content: i32, param: usize, foe: Foe) -> i32 {
        self.orbs.iter().filter(|worn| worn.content == content && worn.strikes(foe)).fold(0, |sum, worn| sum.wrapping_add(worn.value(param)))
    }

    pub fn orb_answers(&self, label: Identity) -> bool {
        let foe = Foe::default().with(label);

        self.orbs.iter().any(|worn| matches!(worn.content, ATTACK_UP | DEFENSE_UP | STRONG_UP | MASSIVE_UP | TOUGH_UP) && self.orb_works(worn) && worn.strikes(foe))
    }

    fn reaches(&self, mask: u32) -> bool {
        TRAITS.into_iter().enumerate().any(|(bit, label)| mask & (1 << bit) != 0 && self.aims_at(label))
    }

    pub fn orb_fits(&self, content: i32, mask: u32) -> bool {
        match content {
            STRONG_UP => self.strong_against != 0 && self.reaches(mask),
            MASSIVE_UP => self.massive_damage != 0 && self.reaches(mask),
            TOUGH_UP => self.resist != 0 && self.reaches(mask),
            _ => true,
        }
    }

    pub fn orb_works(&self, worn: &Worn) -> bool {
        match worn.content {
            COUNTER_SURGE => self.surge_immune == 0 && self.counter_surge == 0,
            COOLDOWN => self.recharge_cut <= 0,
            COLOSSUS => self.colossus_slayer == 0 && worn.value(0) > 0 && worn.value(1) > 0,
            content => self.orb_fits(content, worn.mask),
        }
    }

    pub fn wear(&mut self, worn: Vec<Worn>, plain_attacks: [i32; 3], switches: Switches, mode: Mode) {
        let lit: Vec<&Worn> = worn.iter().filter(|held| held.toggled(switches, mode) && self.orb_works(held)).collect();
        let best = |content: i32| lit.iter().filter(|held| held.content == content).max_by_key(|held| held.grade).map_or(0, |held| held.value(0));

        self.orb_terms = Terms {
            cost_cut: best(COST_DOWN),
            refund: lit.iter().filter(|held| held.content == CASH_BACK).fold(0, |sum, held| sum.wrapping_add(held.value(0))),
            recharge_cut: best(COOLDOWN),
            halved: mode == Mode::Average,
            berserk: best(BERSERKER),
        };
        self.orbs = worn;
        self.plain_attacks = plain_attacks;
    }

    pub fn orb_cost(&self, full: i32) -> i32 {
        let terms = self.orb_terms;

        if terms.cost_cut == 0 && terms.refund == 0 {
            return full;
        }

        let paid = full.wrapping_mul(100 - terms.cost_cut) / 100;
        let kept = paid - paid.wrapping_mul(terms.refund) / 100;

        if terms.halved { (full + kept) / 2 } else { kept }
    }

    pub fn orb_recharge(&self, full: i32) -> i32 {
        let terms = self.orb_terms;

        if terms.recharge_cut == 0 {
            return full;
        }

        let cut = (full.wrapping_mul(100 - terms.recharge_cut) / 100).max(full.min(RECHARGE_FLOOR));

        if terms.halved { (full + cut) / 2 } else { cut }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orb(content: i32, mask: u32, grade: i32, values: &[i32]) -> Worn {
        Worn { slot: 0, id: 0, content, mask, grade, values: values.to_vec() }
    }

    // A slot marked 1 waits for level 60; any other mark, or none, is open from the start.
    #[test]
    fn only_a_marked_slot_waits_for_level_sixty() {
        assert!(!unlocked(1, 59));
        assert!(unlocked(1, 60));
        assert!(unlocked(0, 1));
        assert!(unlocked(2, 1));
    }

    // Two of the same kind: a summing orb counts both, a capped one takes the better grade's value.
    #[test]
    fn sums_stack_and_maxes_take_the_best_grade() {
        let mut unit = Entity::default();

        unit.wear(vec![orb(RESIST_WAVE, 0, 1, &[10]), orb(RESIST_WAVE, 0, 4, &[20]), orb(COST_DOWN, 0, 1, &[5]), orb(COST_DOWN, 0, 3, &[9])], [0; 3], Switches::default(), Mode::Maximum);

        assert_eq!(unit.orb_sum(RESIST_WAVE, 0), 30);
        assert_eq!(unit.orb_max(COST_DOWN, 0), 9);
        assert_eq!(unit.orb_max(DODGE, 0), 0);
    }

    // An orb aimed at Red counts once against a Red enemy, and not at all against an Angel.
    #[test]
    fn a_trait_orb_counts_only_against_its_traits() {
        let mut unit = Entity::default();
        let red = Foe::default().with(Identity::TraitRed);

        unit.wear(vec![orb(ATTACK_UP, 0b1, 4, &[30]), orb(ATTACK_UP, 0b11, 2, &[10])], [0; 3], Switches::default(), Mode::Maximum);

        assert_eq!(unit.orb_versus(ATTACK_UP, 0, red.with(Identity::TraitFloating)), 40);
        assert_eq!(unit.orb_versus(ATTACK_UP, 0, Foe::default().with(Identity::TraitAngel)), 0);
    }

    // A Defense Up orb aimed at Aku answers Aku even when the unit targets nothing.
    #[test]
    fn a_trait_orb_answers_its_trait() {
        let mut unit = Entity::default();

        unit.wear(vec![orb(DEFENSE_UP, 1 << 11, 4, &[20])], [0; 3], Switches::default(), Mode::Average);

        assert!(unit.orb_answers(Identity::TraitAku));
        assert!(!unit.orb_answers(Identity::TraitRed));
    }

    // Ultra Windy has Insane Damage, not Massive, so her True form's orb does nothing on her.
    #[test]
    fn an_ability_orb_needs_the_ability_on_this_form() {
        let floating = 0b10;
        let massive = orb(MASSIVE_UP, floating, 4, &[30]);
        let true_form = Entity { trait_floating: 1, massive_damage: 1, ..Entity::default() };
        let ultra_form = Entity { trait_floating: 1, insane_damage: 1, ..Entity::default() };

        assert!(true_form.orb_works(&massive));
        assert!(!ultra_form.orb_works(&massive));
        assert!(!true_form.orb_fits(MASSIVE_UP, 0b1), "aimed at Red, which she never targets");
        assert!(ultra_form.orb_fits(ATTACK_UP, 0b1), "Attack Up goes on anyone");
    }

    // Cost Down and Cash Back fire on the same deploys, so Max is that deploy and Avg the mean of two.
    #[test]
    fn alternate_deploy_orbs_average_over_two_deploys() {
        let cost = |mode: Mode| {
            let mut unit = Entity::default();
            let worn = vec![Worn { slot: 0, ..orb(COST_DOWN, 0, 4, &[30]) }, Worn { slot: 1, ..orb(CASH_BACK, 0, 4, &[50]) }];

            unit.wear(worn, [0; 3], Switches::default(), mode);
            unit.orb_cost(1000)
        };

        assert_eq!(cost(Mode::Maximum), 350);
        assert_eq!(cost(Mode::Average), 675);
        assert_eq!(cost(Mode::Base), 1000);
    }

    #[test]
    fn berserker_waits_for_max_unless_switched() {
        let mut unit = Entity::default();
        let worn = vec![orb(BERSERKER, 0, 4, &[20])];

        unit.wear(worn.clone(), [0; 3], Switches::default(), Mode::Average);
        assert_eq!(unit.orb_terms.berserk, 0);

        unit.wear(worn, [0; 3], Switches::default().set(Switch::Orb1, true), Mode::Average);
        assert_eq!(unit.orb_terms.berserk, 20);
    }

    #[test]
    fn an_innate_ability_leaves_its_orb_idle() {
        let counter = orb(COUNTER_SURGE, 0, 4, &[100]);
        let colossus = orb(COLOSSUS, 0, 4, &[160, 70]);

        assert!(Entity::default().orb_works(&counter));
        assert!(!Entity { counter_surge: 1, ..Entity::default() }.orb_works(&counter));
        assert!(!Entity { surge_immune: 1, ..Entity::default() }.orb_works(&counter));
        assert!(!Entity { colossus_slayer: 1, ..Entity::default() }.orb_works(&colossus));
        assert!(!Entity { recharge_cut: 10, ..Entity::default() }.orb_works(&orb(COOLDOWN, 0, 4, &[20])));
    }
}
