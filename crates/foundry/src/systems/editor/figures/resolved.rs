use std::fmt;
use crate::domains::settings::EditorMode;

use super::combat::{cats, enemies};
use super::Subject;

use super::{
    mapsheets,
    enigma,
    shrine,
    prizes,
    guide,
    backgrounds, combos, costs, itembuy, mapdata, mapdrops, mapoption, talents, unitbuy, unitlevel,
    altar, castlegrowth, castles, chance, club, combocategories, development, labyrinth, missionsettings, popups, quest, realms, sounds, tiprules, conditions, gacha, gamatoto, missions, progress, upgrades, cannonstatus, cpu, orbtraits, comboparams, difficulty, exlottery, exoption, expcurve, orbgrades, orbslots, recipes,
    shakes, shop, zombies,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Plain,
    Opaque,
    Flag,
    Choice(&'static [Choice]),
    Gated(&'static [Choice], Gate),
    Percent,
    Floor(i32),
    Range(i32, i32),
    Offset(i32),
    Ratio(i32, i32),
    Inverted(i32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gate {
    pub field: &'static str,
    pub blocked: i32,
    pub reason: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Choice {
    pub raw: i32,
    pub label: &'static str,
}

impl Choice {
    pub const fn new(raw: i32, label: &'static str) -> Choice {
        Choice { raw, label }
    }

    pub fn raw(self) -> i32 {
        self.raw
    }

    pub fn label(self) -> &'static str {
        self.label
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Toggle {
    No,
    Yes,
}

impl Toggle {
    pub fn flip(self) -> Toggle {
        match self {
            Toggle::No => Toggle::Yes,
            Toggle::Yes => Toggle::No,
        }
    }

    pub fn raw(self) -> i32 {
        match self {
            Toggle::No => 0,
            Toggle::Yes => 1,
        }
    }

    pub fn of(raw: i32) -> Option<Toggle> {
        match raw {
            0 => Some(Toggle::No),
            1 => Some(Toggle::Yes),
            _ => None,
        }
    }
}

impl fmt::Display for Toggle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Toggle::No => "No",
            Toggle::Yes => "Yes",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Number,
    Danger,
    Toggle(Toggle),
    Choice(&'static [Choice], Choice),
    Disabled(&'static str),
}

pub(super) const PERCENT_FLOOR: i32 = 0;

pub(super) const PERCENT_CEILING: i32 = 100;

impl Rule {
    pub fn gate(self) -> Option<Gate> {
        match self {
            Rule::Gated(_, gate) => Some(gate),
            _ => None,
        }
    }

    pub fn to_display(self, raw: i32, values: EditorMode) -> i32 {
        if values == EditorMode::Raw {
            return raw;
        }

        match self {
            Rule::Offset(by) => raw.saturating_add(by),
            Rule::Ratio(numerator, denominator) => ratio(raw, numerator, denominator),
            Rule::Inverted(base) => base.saturating_sub(raw),
            _ => raw,
        }
    }

    pub fn to_raw(self, display: i32, values: EditorMode) -> i32 {
        if values == EditorMode::Raw {
            return display;
        }

        match self {
            Rule::Offset(by) => display.saturating_sub(by),
            Rule::Ratio(numerator, denominator) => ratio(display, denominator, numerator),
            Rule::Inverted(base) => base.saturating_sub(display),
            _ => display,
        }
    }

    pub fn signed(self, values: EditorMode) -> bool {
        if values == EditorMode::Raw {
            return true;
        }

        match self {
            Rule::Percent | Rule::Ratio(_, _) | Rule::Inverted(_) => false,
            Rule::Floor(least) | Rule::Range(least, _) => least < 0,
            _ => true,
        }
    }

    pub fn clamp(self, raw: i32, values: EditorMode) -> i32 {
        if values == EditorMode::Raw {
            return raw;
        }

        match self {
            Rule::Percent => raw.clamp(PERCENT_FLOOR, PERCENT_CEILING),
            Rule::Inverted(base) => raw.clamp(PERCENT_FLOOR, base),
            Rule::Ratio(_, _) => raw.max(PERCENT_FLOOR),
            Rule::Floor(least) => raw.max(least),
            Rule::Range(least, most) => raw.clamp(least, most),
            _ => raw,
        }
    }

    pub fn face(self, raw: i32, values: EditorMode) -> Face {
        if values == EditorMode::Raw {
            return Face::Number;
        }

        match self {
            Rule::Plain
            | Rule::Percent
            | Rule::Floor(_)
            | Rule::Range(_, _)
            | Rule::Offset(_)
            | Rule::Ratio(_, _)
            | Rule::Inverted(_) => Face::Number,
            Rule::Opaque => Face::Danger,
            Rule::Flag => Toggle::of(raw).map_or(Face::Danger, Face::Toggle),
            Rule::Choice(options) | Rule::Gated(options, _) => options
                .iter()
                .find(|choice| choice.raw == raw)
                .map_or(Face::Danger, |choice| Face::Choice(options, *choice)),
        }
    }
}

fn ratio(value: i32, numerator: i32, denominator: i32) -> i32 {
    let scaled = f64::from(value) * f64::from(numerator) / f64::from(denominator);

    scaled.round() as i32
}

pub(super) fn lookup(field: Option<&str>, find: fn(&str) -> Option<Rule>) -> Rule {
    field.and_then(find).unwrap_or(Rule::Opaque)
}

pub fn note(subject: Subject, field: Option<&str>) -> Option<&'static str> {
    let field = field?;

    match subject {
        Subject::Cat => cats::note(field),
        Subject::Enemy => enemies::note(field),
        Subject::Buy
        | Subject::Curve
        | Subject::Talents
        | Subject::Costs
        | Subject::Combo
        | Subject::MapStage
        | Subject::MapDrops
        | Subject::DropChara
        | Subject::ItemBuy
        | Subject::MapOption
        | Subject::Background
        | Subject::Upgrade
        | Subject::ComboParam
        | Subject::OrbSlots
        | Subject::Difficulty
        | Subject::ExInvasion
        | Subject::CannonStatus
        | Subject::ExpCurve
        | Subject::OrbGrades
        | Subject::ExLottery
        | Subject::Shakes
        | Subject::Recipes
        | Subject::Shop
        | Subject::ZombieHeader
        | Subject::ZombieTier
        | Subject::Cpu
        | Subject::OrbTraits
        | Subject::Altar
        | Subject::GamatotoLimit
        | Subject::GamatotoUnlock
        | Subject::GamatotoDrop
        | Subject::Stamps
        | Subject::SlotUnlocks
        | Subject::Club
        | Subject::Missions
        | Subject::Conditions
        | Subject::MissionUnlocks
        | Subject::MissionGroups
        | Subject::GachaOptions
        | Subject::ChanceAnimations
        | Subject::Castles
        | Subject::TipRules
        | Subject::QuestStages
        | Subject::SlotLottery
        | Subject::RealmAltar
        | Subject::RealmDraws
        | Subject::LabyrinthFloors
        | Subject::LabyrinthLottery
        | Subject::MissionShortcuts
        | Subject::MissionDesignations
        | Subject::MonthlyMissions
        | Subject::SoundSettings
        | Subject::DojoChests
        | Subject::PopupTriggers
        | Subject::ComboCategories
        | Subject::CastleGrowth
        | Subject::DevelopmentCosts
        | Subject::DevelopmentTiers
        | Subject::MaterialMixes
        | Subject::GuideEntries
        | Subject::UnitLimits
        | Subject::EnemyGuide
        | Subject::ExpeditionAreas
        | Subject::GamatotoLevels
        | Subject::PrizePatterns
        | Subject::ShrineOfferings
        | Subject::ShrineExtras
        | Subject::FortuneOdds
        | Subject::ZonedIn
        | Subject::ZonedInSteps
        | Subject::FilterMaps
        | Subject::LineupExclusions
        | Subject::EnigmaLotteries
        | Subject::EnigmaRates
        | Subject::EnigmaDrops
        | Subject::EnigmaGroups
        | Subject::DiscoveryMedals
        | Subject::MapMarkers
        | Subject::EnigmaTimes
        | Subject::RecommendedUpgrades
        | Subject::LineupCounters
        | Subject::OrbOptions
        | Subject::EnergyRefunds
        | Subject::StageShortcuts
        | Subject::FanfareTiers
        | Subject::GuideSpotlights
        | Subject::TipSettings
        | Subject::EnigmaRarities
        | Subject::EngineerCap
        | Subject::LineupRatings
        | Subject::LineupBands
        | Subject::LineupRankGroups
        | Subject::AbilityIcons
        | Subject::TraitIcons
        | Subject::MenuTextSettings
        | Subject::MapSheets
        | Subject::OrbFilterChoices
        | Subject::ReelSymbols
        | Subject::LevelUpOffers
        | Subject::DemonCastles => None,
    }
}

pub fn rule(subject: Subject, index: usize, field: Option<&str>, cells: &[i32]) -> Rule {
    match subject {
        Subject::Cat => lookup(field, cats::rule),
        Subject::Enemy => lookup(field, enemies::rule),
        Subject::Buy => lookup(field, unitbuy::rule),
        Subject::Curve => unitlevel::rule(),
        Subject::Talents => talents::rule(index, cells),
        Subject::Costs => costs::rule(index),
        Subject::Combo => lookup(field, combos::rule),
        Subject::MapStage => mapdata::rule(index, field, cells),
        Subject::MapDrops => mapdrops::rule(index),
        Subject::DropChara => mapdrops::chara_rule(index),
        Subject::ItemBuy => lookup(field, itembuy::rule),
        Subject::MapOption => lookup(field, mapoption::rule),
        Subject::Background => lookup(field, backgrounds::rule),
        Subject::Upgrade => lookup(field, upgrades::rule),
        Subject::ComboParam => comboparams::rule(),
        Subject::OrbSlots => orbslots::rule(index),
        Subject::Difficulty => difficulty::rule(index),
        Subject::ExInvasion => exoption::rule(index),
        Subject::CannonStatus => cannonstatus::rule(index),
        Subject::ExpCurve => expcurve::rule(),
        Subject::OrbGrades => orbgrades::rule(index),
        Subject::ExLottery => exlottery::rule(),
        Subject::Shakes => shakes::rule(index),
        Subject::Recipes => recipes::rule(index),
        Subject::Shop => shop::rule(index),
        Subject::ZombieHeader => zombies::header_rule(index),
        Subject::ZombieTier => zombies::tier_rule(index),
        Subject::Cpu => cpu::rule(index),
        Subject::OrbTraits => orbtraits::rule(),
        Subject::Altar => altar::rule(index),
        Subject::GamatotoLimit => gamatoto::limit_rule(),
        Subject::GamatotoUnlock => gamatoto::unlock_rule(index),
        Subject::GamatotoDrop => gamatoto::drop_rule(index),
        Subject::Stamps => progress::stamp_rule(),
        Subject::SlotUnlocks => progress::slot_rule(),
        Subject::Club => club::rule(index),
        Subject::Missions => missions::rule(index),
        Subject::Conditions => conditions::rule(index),
        Subject::MissionUnlocks => missions::unlock_rule(index),
        Subject::MissionGroups => conditions::group_rule(index),
        Subject::GachaOptions => gacha::rule(index),
        Subject::ChanceAnimations => chance::rule(index),
        Subject::Castles => castles::rule(index),
        Subject::TipRules => tiprules::rule(index),
        Subject::QuestStages => quest::stage_rule(index),
        Subject::SlotLottery => quest::lottery_rule(),
        Subject::RealmAltar => realms::altar_rule(),
        Subject::RealmDraws => realms::draw_rule(index),
        Subject::LabyrinthFloors => labyrinth::rule(),
        Subject::LabyrinthLottery => labyrinth::lottery_rule(index),
        Subject::MissionShortcuts => missionsettings::shortcut_rule(index),
        Subject::MissionDesignations => missionsettings::designation_rule(index),
        Subject::MonthlyMissions => missionsettings::monthly_rule(index),
        Subject::SoundSettings => sounds::sound_rule(index),
        Subject::DojoChests => sounds::chest_rule(index),
        Subject::PopupTriggers => popups::rule(index),
        Subject::ComboCategories => combocategories::rule(index),
        Subject::CastleGrowth => castlegrowth::rule(index),
        Subject::DemonCastles => realms::castle_rule(index),
        Subject::LevelUpOffers => guide::level_up_rule(index),
        Subject::ReelSymbols => prizes::reel_rule(),
        Subject::OrbFilterChoices => guide::orb_choice_rule(index),
        Subject::MapSheets => mapsheets::rule(index),
        Subject::MenuTextSettings => guide::menu_text_rule(index),
        Subject::TraitIcons => guide::trait_icon_rule(index),
        Subject::AbilityIcons => guide::ability_icon_rule(index),
        Subject::LineupRankGroups => guide::rank_group_rule(index),
        Subject::LineupBands => guide::listed_rule(),
        Subject::LineupRatings => guide::rating_rule(index),
        Subject::EngineerCap => development::cost_rule(),
        Subject::EnigmaRarities => enigma::rarity_rule(index),
        Subject::TipSettings => guide::tip_setting_rule(index),
        Subject::GuideSpotlights => guide::listed_rule(),
        Subject::FanfareTiers => guide::listed_rule(),
        Subject::StageShortcuts => guide::shortcut_rule(index),
        Subject::EnergyRefunds => guide::refund_rule(index),
        Subject::OrbOptions => guide::orb_option_rule(index),
        Subject::LineupCounters => guide::counter_rule(index),
        Subject::RecommendedUpgrades => guide::recommend_rule(index),
        Subject::EnigmaTimes => enigma::plain_rule(),
        Subject::MapMarkers => guide::marker_rule(index),
        Subject::DiscoveryMedals => enigma::plain_rule(),
        Subject::EnigmaGroups => enigma::group_rule(index),
        Subject::EnigmaDrops => enigma::plain_rule(),
        Subject::EnigmaRates => enigma::rate_rule(index),
        Subject::EnigmaLotteries => enigma::plain_rule(),
        Subject::LineupExclusions => guide::listed_rule(),
        Subject::FilterMaps => guide::listed_rule(),
        Subject::ZonedInSteps => gamatoto::zone_step_rule(index),
        Subject::ZonedIn => gamatoto::zone_rule(),
        Subject::FortuneOdds => shrine::fortune_rule(index),
        Subject::ShrineExtras => shrine::extra_rule(index),
        Subject::ShrineOfferings => shrine::offering_rule(index),
        Subject::PrizePatterns => prizes::pattern_rule(index),
        Subject::GamatotoLevels => gamatoto::level_rule(),
        Subject::ExpeditionAreas => gamatoto::area_rule(index),
        Subject::EnemyGuide => guide::enemy_rule(index),
        Subject::UnitLimits => guide::limit_rule(),
        Subject::GuideEntries => guide::guide_rule(index),
        Subject::MaterialMixes => development::mix_rule(index),
        Subject::DevelopmentTiers => development::tier_rule(index),
        Subject::DevelopmentCosts => development::cost_rule(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domains::settings::EditorMode;
    use crate::systems::editor::figures::combat::{cats, enemies};
    use crate::systems::editor::figures::{schema, SUBJECTS};

    use super::{rule, Rule};

    #[test]
    fn a_quartered_range_is_typed_at_the_distance_it_shows() {
        let rule = Rule::Ratio(1, 4);

        assert_eq!(rule.to_display(1000, EditorMode::Resolved), 250);
        assert_eq!(rule.to_raw(250, EditorMode::Resolved), 1000);
        assert_eq!(rule.to_display(1000, EditorMode::Raw), 1000, "raw stays raw");
    }

    #[test]
    fn a_cost_is_typed_in_the_money_it_is_worth() {
        let rule = Rule::Ratio(3, 2);

        assert_eq!(rule.to_display(50, EditorMode::Resolved), 75);
        assert_eq!(rule.to_raw(75, EditorMode::Resolved), 50);
    }

    #[test]
    fn an_inverted_percent_reverses_in_both_directions() {
        let rule = Rule::Inverted(100);

        assert_eq!(rule.to_display(30, EditorMode::Resolved), 70);
        assert_eq!(rule.to_raw(70, EditorMode::Resolved), 30);
        assert_eq!(rule.clamp(140, EditorMode::Resolved), 100, "a percent cannot exceed its base");
    }

    #[test]
    fn cats_and_enemies_agree_on_the_columns_they_share() {
        for field in ["area_attack", "attack_count_state"] {
            assert_eq!(
                cats::rule(field),
                enemies::rule(field),
                "{field} is declared in both combat tables and the two copies have drifted",
            );
        }
    }

    #[test]
    fn no_scaled_column_is_a_flag() {
        for subject in SUBJECTS {
            for (index, entry) in schema::of(subject).order().iter().enumerate() {
                if entry.scale == codex::combat::Scale::Raw {
                    continue;
                }

                assert_ne!(
                    rule(subject, index, Some(entry.field), &[]),
                    Rule::Flag,
                    "{subject:?}: {} carries a codex Scale, so it is a magnitude and never a flag",
                    entry.field,
                );
            }
        }
    }
}
