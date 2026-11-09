use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TreasureEffect {
    BaseHealth,
    CannonRecharge,
    CannonPower,
    WorkerEfficiency,
    WalletCapacity,
    EnemyMoney,
    Experience,
    EnergyRecovery,
    MaxEnergy,
    CatHealth,
    CatAttack,
    CatRecharge,
    Red,
    Dark,
    Floating,
    Angel,
    AlienWeakening,
    CannonRangeRecharge,
    StarredAlienWeakening,
    Metal,
    Zombie,
    Alien,
    StarredAlienType2,
    StarredAlienType3,
    StarredAlienType4,
    Unknown(i32),
}

impl From<i32> for TreasureEffect {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::BaseHealth,
            1 => Self::CannonRecharge,
            2 => Self::CannonPower,
            3 => Self::WorkerEfficiency,
            4 => Self::WalletCapacity,
            5 => Self::EnemyMoney,
            6 => Self::Experience,
            7 => Self::EnergyRecovery,
            8 => Self::MaxEnergy,
            9 => Self::CatHealth,
            10 => Self::CatAttack,
            11 => Self::CatRecharge,
            12 => Self::Red,
            13 => Self::Dark,
            14 => Self::Floating,
            15 => Self::Angel,
            16 => Self::AlienWeakening,
            17 => Self::CannonRangeRecharge,
            18 => Self::StarredAlienWeakening,
            19 => Self::Metal,
            20 => Self::Zombie,
            21 => Self::Alien,
            22 => Self::StarredAlienType2,
            23 => Self::StarredAlienType3,
            24 => Self::StarredAlienType4,
            other => Self::Unknown(other),
        }
    }
}

impl TreasureEffect {
    pub fn id(self) -> i32 {
        match self {
            Self::BaseHealth => 0,
            Self::CannonRecharge => 1,
            Self::CannonPower => 2,
            Self::WorkerEfficiency => 3,
            Self::WalletCapacity => 4,
            Self::EnemyMoney => 5,
            Self::Experience => 6,
            Self::EnergyRecovery => 7,
            Self::MaxEnergy => 8,
            Self::CatHealth => 9,
            Self::CatAttack => 10,
            Self::CatRecharge => 11,
            Self::Red => 12,
            Self::Dark => 13,
            Self::Floating => 14,
            Self::Angel => 15,
            Self::AlienWeakening => 16,
            Self::CannonRangeRecharge => 17,
            Self::StarredAlienWeakening => 18,
            Self::Metal => 19,
            Self::Zombie => 20,
            Self::Alien => 21,
            Self::StarredAlienType2 => 22,
            Self::StarredAlienType3 => 23,
            Self::StarredAlienType4 => 24,
            Self::Unknown(value) => value,
        }
    }

    pub fn for_enemy(trait_alien: bool, trait_starred_alien: i32) -> Option<Self> {
        match (trait_alien, trait_starred_alien) {
            (true, 0) => Some(Self::AlienWeakening),
            (true, 1) => Some(Self::StarredAlienWeakening),
            (_, 2) => Some(Self::StarredAlienType2),
            (_, 3) => Some(Self::StarredAlienType3),
            (_, 4) => Some(Self::StarredAlienType4),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraitBonus(pub i32);

impl TraitBonus {
    pub fn strong_dealt(self, raise: i32) -> f64 {
        f64::from(1500 + self.0 + raise * 10) / 1000.0
    }

    pub fn strong_taken(self, raise: i32) -> f64 {
        f64::from(1500 - self.0) / 3000.0 * f64::from(100 - raise) / 100.0
    }

    pub fn resist_taken(self, raise: i32) -> f64 {
        f64::from(1500 - self.0) / 6000.0 * f64::from(100 - raise) / 100.0
    }

    pub fn massive_dealt(self, raise: i32) -> f64 {
        f64::from(900 + self.0 + raise * 3) / 300.0
    }

    pub fn insane_dealt(self) -> f64 {
        f64::from(1500 + self.0) / 300.0
    }

    pub fn insanely_tough_taken(self) -> f64 {
        f64::from(2100 - self.0) / 12600.0
    }

    pub fn duration(self) -> f64 {
        f64::from(1500 + self.0) / 1500.0
    }

    pub fn scale_duration(self, frames: i32, raise: i32) -> i32 {
        let fruited = i64::from(frames) * i64::from(1500 + self.0) / 1500;

        (fruited * i64::from(100 + raise) / 100) as i32
    }
}

pub fn enemy_money(drop: i32, accounting: i32, treasure: i32) -> i32 {
    let scaled = i64::from(accounting * 5 + treasure + 100) * i64::from(drop) / 100;

    scaled.max(0) as i32
}

pub fn unit_recharge(cooldown: i32, research: i32, treasure: i32) -> i32 {
    let reduced = cooldown - (treasure + research * 6);

    if reduced > 60 { reduced } else { 60 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_identifier_round_trips() {
        for id in 0..=24 {
            assert_eq!(TreasureEffect::from(id).id(), id);
        }

        assert_eq!(TreasureEffect::from(99), TreasureEffect::Unknown(99));
    }

    #[test]
    fn a_full_set_of_fruit_lands_on_the_familiar_multipliers() {
        let bonus = TraitBonus(300);

        assert_eq!(bonus.strong_dealt(0), 1.8);
        assert_eq!(bonus.massive_dealt(0), 4.0);
        assert_eq!(bonus.resist_taken(0), 0.2);
        assert_eq!(TraitBonus(0).insanely_tough_taken(), 1.0 / 6.0);
    }

    #[test]
    fn a_duration_truncates_the_way_the_engine_does() {
        assert_eq!(TraitBonus(300).scale_duration(150, 0), 180);
        assert_eq!(TraitBonus(300).scale_duration(38, 0), 45);
        assert_eq!(TraitBonus(0).scale_duration(38, 0), 38);
    }

    // A run's effect boost stacks on the fruit: added to what is dealt, taken off what is left to take.
    #[test]
    fn a_run_boost_rides_on_top_of_the_fruit() {
        let bonus = TraitBonus(300);

        assert_eq!(bonus.strong_dealt(20), 2.0);
        assert_eq!(bonus.massive_dealt(100), 5.0);
        assert_eq!(bonus.strong_taken(50), 0.2);
        assert_eq!(bonus.resist_taken(50), 0.1);
        assert_eq!(bonus.scale_duration(38, 50), 67, "the fruit truncates first, then the boost");
    }

    #[test]
    fn maxed_accounting_and_treasure_pay_three_point_nine_five() {
        assert_eq!(enemy_money(100, 29, 150), 395);
        assert_eq!(unit_recharge(400, 29, 90), 136);
        assert_eq!(unit_recharge(100, 29, 90), 60);
    }
}
