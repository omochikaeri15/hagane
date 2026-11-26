mod catalog;
mod config;

use codex::chapter::stage::CertificationPreset;
use codex::chapter::treasure::{enemy_money, unit_recharge, TraitBonus, TreasureEffect, TreasureLevels, TreasureProgress, ARC_SLOTS, SLOTS, STAGES};
use codex::chapter::Category;
use codex::combat::{crowned, Entity, Faction, Foe, Identity};

pub use catalog::Catalog;
pub use config::{Config, ARCS, FULL_PERCENT};

const MAX_TECH: i32 = 29;
const FRUIT_PERCENT: i32 = 100;

const FRUITS: [TreasureEffect; 7] = [
    TreasureEffect::Red,
    TreasureEffect::Floating,
    TreasureEffect::Dark,
    TreasureEffect::Angel,
    TreasureEffect::Metal,
    TreasureEffect::Zombie,
    TreasureEffect::Alien,
];

const FRUIT_LABELS: [Identity; 7] = [
    Identity::TraitRed,
    Identity::TraitFloating,
    Identity::TraitDark,
    Identity::TraitAngel,
    Identity::TraitMetal,
    Identity::TraitZombie,
    Identity::TraitAlien,
];

const WEAKENINGS: [TreasureEffect; 5] = [
    TreasureEffect::AlienWeakening,
    TreasureEffect::StarredAlienWeakening,
    TreasureEffect::StarredAlienType2,
    TreasureEffect::StarredAlienType3,
    TreasureEffect::StarredAlienType4,
];

const OUTSIDE_STORY: usize = SLOTS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub(crate) cat_health: i32,
    pub(crate) cat_attack: i32,
    pub(crate) enemy_money: i32,
    pub(crate) cat_recharge: i32,
    fruits: [i32; FRUITS.len()],
    pushes: [i32; FRUITS.len()],
    weakenings: [[i32; WEAKENINGS.len()]; SLOTS + 1],
}

impl Bonus {
    pub fn resolve(catalog: &Catalog, config: &Config) -> Self {
        Self::from_levels(catalog, &config.levels())
    }

    pub fn from_levels(catalog: &Catalog, levels: &TreasureLevels) -> Self {
        Self::from_progress(catalog, &catalog.treasures.progress(levels))
    }

    pub fn preset(catalog: &Catalog, levels: &TreasureLevels, preset: &CertificationPreset) -> Self {
        let treasures = &catalog.treasures;

        Self::from_progress(catalog, &preset.treasure_progress(treasures, &treasures.progress(levels)))
    }

    fn from_progress(catalog: &Catalog, progress: &TreasureProgress) -> Self {
        let treasures = &catalog.treasures;
        let value = |effect| treasures.value_at(progress, effect, None);

        let mut weakenings = [[0; WEAKENINGS.len()]; SLOTS + 1];

        for (index, row) in weakenings.iter_mut().enumerate() {
            let chapter = (index != OUTSIDE_STORY).then_some(index);

            for (gap, effect) in row.iter_mut().zip(WEAKENINGS) {
                *gap = treasures.uncapped(effect, chapter) - treasures.value_at(progress, effect, chapter);
            }
        }

        Self {
            cat_health: value(TreasureEffect::CatHealth),
            cat_attack: value(TreasureEffect::CatAttack),
            enemy_money: value(TreasureEffect::EnemyMoney),
            cat_recharge: value(TreasureEffect::CatRecharge),
            fruits: FRUITS.map(|effect| treasures.capped_at(progress, effect, FRUIT_PERCENT, None)),
            pushes: FRUITS.map(|effect| treasures.value_at(progress, effect, None)),
            weakenings,
        }
    }

    pub fn fruit(&self, effect: TreasureEffect) -> TraitBonus {
        TraitBonus(FRUITS.iter().position(|fruit| *fruit == effect).map_or(0, |index| self.fruits[index]))
    }

    pub fn weakening(&self, stats: &Entity, chapter: Option<usize>) -> i32 {
        let Some(effect) = TreasureEffect::for_enemy(stats.trait_alien != 0, stats.trait_starred_alien) else {
            return 0;
        };

        let row = chapter.filter(|chapter| *chapter < SLOTS).unwrap_or(OUTSIDE_STORY);

        WEAKENINGS.iter().position(|weakening| *weakening == effect).map_or(0, |index| self.weakenings[row][index])
    }

    pub fn fruit_traits(&self, stats: &Entity) -> Vec<(Identity, TraitBonus)> {
        let bare = [Identity::TraitTraitless, Identity::TraitWitch, Identity::TraitEva, Identity::TraitRelic, Identity::TraitAku];

        FRUIT_LABELS
            .iter()
            .zip(self.fruits)
            .map(|(label, value)| (*label, value))
            .chain(bare.into_iter().map(|label| (label, 0)))
            .filter(|(label, _)| stats.aims_at(*label))
            .map(|(label, value)| (label, TraitBonus(value)))
            .collect()
    }

    pub fn fruit_span(&self, stats: &Entity) -> (TraitBonus, TraitBonus) {
        let (low, high) = spread(stats, &self.fruits);

        (TraitBonus(low), TraitBonus(high))
    }

    pub fn push_span(&self, stats: &Entity) -> (i32, i32) {
        match stats.faction {
            Faction::Cat => spread(stats, &self.pushes),
            Faction::Enemy => (0, 0),
        }
    }

    pub fn push_fruit(&self, foe: Foe) -> i32 {
        self.pushes.iter().zip(FRUIT_LABELS).filter(|(_, label)| foe.has(*label)).map(|(value, _)| *value).max().unwrap_or(0).max(0)
    }

    pub fn fruit_against(&self, stats: &Entity, foe: Foe) -> TraitBonus {
        let shared = FRUITS.iter().zip(FRUIT_LABELS).filter(|(_, label)| foe.has(*label) && stats.aims_at(*label));

        TraitBonus(shared.map(|(effect, _)| self.fruit(*effect).0).max().unwrap_or(0).max(0))
    }

    pub fn fruit_frames(&self, stats: &Entity, frames: i32, raise: i32) -> (i32, i32) {
        if stats.faction != Faction::Cat {
            return (frames, frames);
        }

        let (low, high) = self.fruit_span(stats);

        (low.scale_duration(frames, raise), high.scale_duration(frames, raise))
    }

    pub(crate) fn scaled(&self, stats: &Entity, stat: i32, percent: i32, crown: i32) -> i32 {
        match stats.faction {
            Faction::Enemy => crowned(stat, percent, crown, self.weakening(stats, None)),
            Faction::Cat => stat,
        }
    }

    pub fn money(&self, drop: i32) -> i32 {
        enemy_money(drop, MAX_TECH, self.enemy_money)
    }

    pub fn recharge(&self, cooldown: i32) -> i32 {
        unit_recharge(cooldown, MAX_TECH, self.cat_recharge)
    }
}

fn spread(stats: &Entity, values: &[i32; FRUITS.len()]) -> (i32, i32) {
    let fruit_flags = [stats.trait_red, stats.trait_floating, stats.trait_dark, stats.trait_angel, stats.trait_metal, stats.trait_zombie, stats.trait_alien];
    let bare_flags = [stats.trait_traitless, stats.trait_witch, stats.trait_eva, stats.trait_relic, stats.trait_aku];
    let held: Vec<i32> = values.iter().zip(fruit_flags).filter(|(_, flag)| *flag != 0).map(|(value, _)| *value).chain(bare_flags.into_iter().filter(|flag| *flag != 0).map(|_| 0)).collect();

    (held.iter().copied().min().unwrap_or(0), held.iter().copied().max().unwrap_or(0))
}

pub fn recorded_levels(rows: &[Vec<i32>]) -> TreasureLevels {
    let mut levels = [[0; STAGES]; SLOTS];

    for (row, recorded) in levels.iter_mut().zip(rows) {
        for (level, held) in row.iter_mut().zip(recorded) {
            *level = *held;
        }
    }

    levels
}

pub fn chapter(category: &Category, map_id: u32) -> Option<usize> {
    let arc = match category {
        Category::EmpireOfCats => 0,
        Category::IntoTheFuture => 1,
        Category::CatsOfTheCosmos => 2,
        _ => return None,
    };
    let map = usize::try_from(map_id).ok()?;

    ARC_SLOTS[arc].contains(&map).then_some(map)
}
