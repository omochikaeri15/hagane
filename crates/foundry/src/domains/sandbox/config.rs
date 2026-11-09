use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::systems::treasure;

use super::jailbreak::Runs;

pub const FILIBUSTER_MAP: u32 = 9;
pub const UNAVAILABLE: &str = "Unavailable";
pub const CAT_GOD_HINT: &str = "Absent: not unlocked, so his button never appears\nPresent: unlocked, miracles at full price\nDiscounted: the 70% off reward for clearing Cats of the Cosmos chapter 1";
pub const NO_CAT_GOD_HINT: &str = "This stage has no Cat God.\nHe appears on Empire of Cats, Into the Future and Cats of the Cosmos stages, outside of outbreaks and invasions";
pub const ALTAR_HINT: &str = "The level the Aku Altar holds this stage's lineup to.\nEmpty: the altar is already destroyed, so nothing is capped";
pub const NO_ALTAR_HINT: &str = "This stage's base is not an Aku Altar, so there is no cap to set";
pub const WAR_FUNDS_HINT: &str = "The War Funds saved up from earlier battles, from 0 to 999999.\nThey are added to the money this stage's trust fund starts with";
pub const NO_WAR_FUNDS_HINT: &str = "This stage does not draw on War Funds";
pub const EXPENSES_HINT: &str = "The Expenses saved up from earlier battles, from 0 to 999999.\nThey are added to the money this stage's trust fund starts with";
pub const NO_EXPENSES_HINT: &str = "This stage does not draw on Expenses";
const FUND_CAP: i32 = 999_999;
const FUND_DIGITS: usize = 6;
const BIG_BANG: u32 = 47;

pub const ITEMS: [&str; 6] = ["Speed Up", "Treasure Radar", "Rich Cat", "Cat CPU", "Cat Jobs", "Sniper the Cat"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tech {
    pub name: &'static str,
    pub slot: usize,
    pub level: u32,
    pub plus: u32,
}

pub const TECHS: [Tech; 10] = [
    Tech { name: "Cat Cannon Attack", slot: 1, level: 20, plus: 10 },
    Tech { name: "Cat Cannon Range", slot: 2, level: 10, plus: 0 },
    Tech { name: "Cat Cannon Charge", slot: 3, level: 20, plus: 10 },
    Tech { name: "Worker Cat Efficiency", slot: 4, level: 20, plus: 10 },
    Tech { name: "Worker Cat Wallet", slot: 5, level: 20, plus: 10 },
    Tech { name: "Cat Base Health", slot: 6, level: 20, plus: 10 },
    Tech { name: "Research", slot: 7, level: 20, plus: 10 },
    Tech { name: "Accounting", slot: 8, level: 20, plus: 10 },
    Tech { name: "Study", slot: 9, level: 20, plus: 10 },
    Tech { name: "Cat Energy", slot: 10, level: 20, plus: 10 },
];

impl Tech {
    pub fn hint(&self) -> String {
        if self.plus > 0 { format!("{}+{}", self.level, self.plus) } else { self.level.to_string() }
    }

    pub fn resolve(&self, entry: &str) -> (u32, u32) {
        let mut terms = entry.split('+').map(|term| term.trim().parse::<u32>().ok());

        let Some(Some(level)) = terms.next() else {
            return (self.level, self.plus);
        };

        let plus = terms.flatten().sum::<u32>();

        (level.clamp(1, self.level), plus.min(self.plus))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatGod {
    Absent,
    #[default]
    Present,
    Discounted,
}

impl CatGod {
    pub const ALL: [Self; 3] = [Self::Absent, Self::Present, Self::Discounted];
}

impl std::fmt::Display for CatGod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Absent => "Absent",
            Self::Present => "Present",
            Self::Discounted => "Discounted",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartSpeed {
    #[default]
    Single,
    Double,
    Triple,
}

impl StartSpeed {
    pub const SELECTABLE: [Self; 2] = [Self::Single, Self::Double];

    pub fn engaged(self) -> bool {
        self != Self::Single
    }
}

impl std::fmt::Display for StartSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Single => "1x",
            Self::Double => "2x",
            Self::Triple => "3x",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tutorial {
    pub battle_cleared: bool,
    pub deck_seen: bool,
    pub two_rows_seen: bool,
    pub cat_god_seen: bool,
    pub shop_seen: bool,
}

impl Default for Tutorial {
    fn default() -> Self {
        Self { battle_cleared: true, deck_seen: true, two_rows_seen: true, cat_god_seen: true, shop_seen: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Invasions {
    pub filibuster: u32,
    pub filibuster_outbreak: u32,
}

impl Default for Invasions {
    fn default() -> Self {
        Self { filibuster: BIG_BANG, filibuster_outbreak: BIG_BANG }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub treasure: treasure::Config,
    pub techs: [String; 10],
    pub cannon: Option<i32>,
    pub style: Option<i32>,
    pub foundation: Option<i32>,
    pub cannon_level: String,
    pub style_level: String,
    pub foundation_level: String,
    pub castle_level: String,
    pub items_off: [bool; 6],
    pub start_speed: StartSpeed,
    pub altar_level: String,
    pub war_funds: String,
    pub expenses: String,
    pub cat_god: CatGod,
    pub tutorial: Tutorial,
    pub invasions: Invasions,
    pub jailbreak: BTreeMap<i32, Runs>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            treasure: treasure::Config::default(),
            techs: Default::default(),
            cannon: None,
            style: None,
            foundation: None,
            cannon_level: String::new(),
            style_level: String::new(),
            foundation_level: String::new(),
            castle_level: String::new(),
            items_off: [true; 6],
            start_speed: StartSpeed::Single,
            altar_level: String::new(),
            war_funds: String::new(),
            expenses: String::new(),
            cat_god: CatGod::default(),
            tutorial: Tutorial::default(),
            invasions: Invasions::default(),
            jailbreak: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn altar(&self) -> Option<i32> {
        self.altar_level.trim().parse::<i32>().ok().map(|level| level.max(1))
    }

    pub fn war_funds(&self) -> i32 {
        fund(&self.war_funds)
    }

    pub fn expenses(&self) -> i32 {
        fund(&self.expenses)
    }

    pub fn castle(&self, highest: i32) -> i32 {
        level(&self.castle_level, highest)
    }

    pub fn tech(&self, index: usize) -> (u32, u32) {
        TECHS.get(index).map_or((1, 0), |tech| tech.resolve(self.techs.get(index).map_or("", String::as_str)))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gimmicks {
    pub cat_god: bool,
    pub altar: bool,
    pub war_funds: bool,
    pub expenses: bool,
    pub jailbreak: bool,
}

fn fund(entry: &str) -> i32 {
    entry.trim().parse::<i32>().map_or(0, |funds| funds.clamp(0, FUND_CAP))
}

pub fn fund_typable(entry: &str) -> bool {
    entry.len() <= FUND_DIGITS && entry.bytes().all(|glyph| glyph.is_ascii_digit())
}

pub fn level(entry: &str, highest: i32) -> i32 {
    entry.trim().parse::<i32>().map_or(highest, |level| level.clamp(1, highest.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // An untouched field has to mean the vanilla cap, not zero.
    #[test]
    fn an_empty_entry_is_the_games_maximum() {
        let config = Config::default();

        assert_eq!(config.tech(0), (20, 10));
        assert_eq!(config.tech(1), (10, 0), "cannon range stops at ten and has no plus levels");
        assert_eq!(level("", 30), 30);
        assert_eq!(config.altar(), None, "no entry means the altar is already destroyed");
        assert_eq!(config.war_funds(), 0, "no entry means nothing saved up yet");
        assert_eq!(config.expenses(), 0);
        // A fresh Sandbox starts with no battle items, so nothing is silently
        // buffing a stage the user is trying to read straight.
        assert_eq!(config.items_off, [true; 6]);
        // Triple is never stored, only shown, so the picker must not offer it.
        assert_eq!(config.start_speed, StartSpeed::Single);
        assert!(!StartSpeed::SELECTABLE.contains(&StartSpeed::Triple));
        assert_eq!(config.cat_god, CatGod::Present);
    }

    #[test]
    fn an_entry_past_the_cap_is_pulled_back() {
        let mut config = Config::default();

        config.techs[0] = "99+99".to_owned();

        assert_eq!(config.tech(0), (20, 10));
        assert_eq!(level("45", 30), 30);

        config.war_funds = "1000000".to_owned();
        config.expenses = "1000000".to_owned();

        assert_eq!(config.war_funds(), FUND_CAP, "a hand-edited config still stops at the game's cap");
        assert_eq!(config.expenses(), FUND_CAP);
        assert!(fund_typable("999999"));
        assert!(!fund_typable("1000000"), "the field never holds more digits than the cap");
        assert!(!fund_typable("1+1"), "the tech level syntax means nothing for a sum of money");
    }
}
