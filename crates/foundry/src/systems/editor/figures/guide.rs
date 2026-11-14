use super::resolved::{Choice, Rule};

pub const GUIDE_NOTICE: &str = "This unit's Cat Guide entry\nThe display sizes scale the model the guide draws for each form (in percent)\nA hidden description shows the guide's shared placeholder text instead of the unit's own\nThe game copies whether the second cell is 0 into its unit lists but never reads it there, so it does nothing";

pub const LIMIT_NOTICE: &str = "The cap groups this unit belongs to\nA User Rank reward that names a group raises the cap of every unit in it\n1000 to 1599 raise the level cap, 4000 to 4599 raise the plus level cap\nThe list ends at the first -1";

pub const GUIDE_LABELS: [&str; 8] = [
    "In Cat Guide",
    "Unused Flag",
    "Forms",
    "Hidden Description",
    "Display Size 1 (%)",
    "Display Size 2 (%)",
    "Display Size 3 (%)",
    "Display Size 4 (%)",
];

pub const LIMIT_LABELS: [&str; 10] = [
    "Cap Group 1",
    "Cap Group 2",
    "Cap Group 3",
    "Cap Group 4",
    "Cap Group 5",
    "Cap Group 6",
    "Cap Group 7",
    "Cap Group 8",
    "Cap Group 9",
    "Cap Group 10",
];

const LISTED: usize = 0;

const UNUSED: usize = 1;

pub(super) const FORMS: usize = 2;

const MYSTERY: usize = 3;

const FORM_CEILING: i32 = 4;

const NO_GROUP: i32 = -1;

const MYSTERIES: [Choice; 3] = [Choice::new(0, "Never"), Choice::new(1, "Until Obtained, And Third Form"), Choice::new(2, "Third Form Only")];

pub(super) fn guide_rule(index: usize) -> Rule {
    match index {
        LISTED => Rule::Flag,
        UNUSED => Rule::Opaque,
        FORMS => Rule::Range(0, FORM_CEILING),
        MYSTERY => Rule::Choice(&MYSTERIES),
        _ => Rule::Floor(0),
    }
}

pub(super) fn limit_rule() -> Rule {
    Rule::Floor(NO_GROUP)
}

pub const ENEMY_NOTICE: &str = "This enemy's place in the Enemy Guide\nThe display size scales the model the guide draws (in percent)\nA map makes the entry show up only while that map is open\n-1 or nothing always lists it";

pub const ENEMY_LABELS: [&str; 3] = ["Enemy ID", "Display Size (%)", "Only While Map Open"];

const NO_MAP: i32 = -1;

pub(super) fn enemy_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        1 => Rule::Floor(0),
        _ => Rule::Floor(NO_MAP),
    }
}

pub const FILTER_NOTICE: &str = "A map the stage list's filters can show\nWhile a filter is on, any map not listed here stays hidden";

pub const EXCLUSION_NOTICE: &str = "An enemy the automatic lineup ignores when it looks at a stage's enemies (just like it ignores the enemy base)";

pub const FILTER_LABELS: [&str; 1] = ["Map ID"];

pub const EXCLUSION_LABELS: [&str; 1] = ["Enemy ID"];

pub(super) fn listed_rule() -> Rule {
    Rule::Floor(0)
}

pub const MARKER_NOTICE: &str = "Where one stage sits on the chapter's world map\nRows follow the order stages are shown in, not their numbers\nThe game loads the last two cells but never reads them";

pub const MARKER_LABELS: [&str; 4] = ["X", "Y", "Unread 1", "Unread 2"];

const MARKER_AXES: usize = 2;

pub(super) fn marker_rule(index: usize) -> Rule {
    match index < MARKER_AXES {
        true => Rule::Plain,
        false => Rule::Opaque,
    }
}

pub const RECOMMEND_NOTICE: &str = "One suggestion on the beginner's power-up list, which is offered below User Rank 1600\nA row applies while its unit or base upgrade is under the level\nRows are read in file order, and the first one that applies sets how many suggestions are shown";

pub const COUNTER_NOTICE: &str = "One pairing the automatic lineup scores\nSets something the stage's enemies have, and the unit ability that counters it";

pub const RECOMMEND_LABELS: [&str; 4] = ["Suggests", "Unit ID Or Base Upgrade", "Until Level", "Suggestions Shown"];

const SUGGESTIONS: [Choice; 2] = [Choice::new(0, "Unit"), Choice::new(1, "Base Upgrade")];

pub(super) fn recommend_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Choice(&SUGGESTIONS),
        _ => Rule::Floor(0),
    }
}

pub const COUNTER_LABELS: [&str; 3] = ["Enemy Has", "Enemy Trait Or Ability ID", "Unit Ability ID"];

pub const ORB_CHOICE_NOTICE: &str = "One choice of the orb filter\nChoices are laid out by group in the order 3, 2, 0, 1, then by the order cell inside a group\nThe value is what the choice matches on an orb\nGroups 0 and 1 match its effect, group 2 matches one of its traits (as the trait's bit number), and group 3 matches its grade\nThe icon is a cut of the filter sheet, and -1 draws cut 50\nThe selected cut is the button drawn while the choice is on, and -1 uses the group's own";

pub const ORB_CHOICE_LABELS: [&str; 5] = ["Group", "Value Matched", "Order", "Icon Cut", "Selected Button Cut"];

const ORB_CHOICE_GROUPS: i32 = 3;

pub(super) fn orb_choice_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Range(0, ORB_CHOICE_GROUPS),
        1 | 2 => Rule::Floor(0),
        _ => Rule::Floor(NO_MAP),
    }
}

const KINDS: [Choice; 2] = [Choice::new(0, "Trait"), Choice::new(1, "Ability")];

pub(super) fn counter_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Choice(&KINDS),
        _ => Rule::Floor(0),
    }
}

pub const LEVEL_UP_NOTICE: &str = "While the scheduled event with this ID runs, the game suggests owned units to level up and discounts their level-up cost\nA unit qualifies while it can still be leveled and its base level is under the limit\nThe game ranks the units that qualify, keeps the top ones as a pool (Pool Size), and shows random picks from it\nWith Also Units With A Form ID on, a unit with a True Form ID from 15000 to 16999 or an Ultra Form ID from 25000 to 26999 in the Unit Buy table also qualifies\nThe game loads the Unused cell but never reads it";

pub const LEVEL_UP_LABELS: [&str; 7] = ["Event ID", "Units Shown", "Discount Percent", "Under Base Level", "Also Units With A Form ID", "Unused", "Pool Size"];

const FULL_PERCENT: i32 = 100;

pub(super) fn level_up_rule(index: usize) -> Rule {
    match index {
        0 | 5 => Rule::Opaque,
        2 => Rule::Range(0, FULL_PERCENT),
        4 => Rule::Flag,
        _ => Rule::Floor(0),
    }
}

pub const ORB_OPTION_NOTICE: &str = "How one orb ability is drawn on a deck button\nDecides if it shows an icon at all, and if the same icon can show up more than once";

pub const ORB_OPTION_LABELS: [&str; 3] = ["Orb Ability ID", "Shows Deck Icon", "Icon May Repeat"];

pub(super) fn orb_option_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Flag,
    }
}

pub const REFUND_NOTICE: &str = "A map whose stages give their energy back after a loss\nA stage of -1 covers the whole map\nA map can be listed more than once to name single stages";

pub const REFUND_LABELS: [&str; 2] = ["Map ID", "Stage"];

pub(super) fn refund_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Floor(0),
        _ => Rule::Floor(NO_MAP),
    }
}

pub const SHORTCUT_NOTICE: &str = "A stage the game jumps to from a mission or an item\nRows sharing an ID are tried in order, and the first open one wins\nA stage of -1 takes the furthest open stage, a crown of -1 takes the highest";

pub const SHORTCUT_LABELS: [&str; 5] = ["Opened From", "Mission Or Item ID", "Map ID", "Stage", "Crown"];

const SHORTCUT_SOURCES: [Choice; 2] = [Choice::new(0, "Mission"), Choice::new(1, "Item")];

pub(super) fn shortcut_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Choice(&SHORTCUT_SOURCES),
        1 | 2 => Rule::Floor(0),
        _ => Rule::Floor(NO_MAP),
    }
}

pub const FANFARE_NOTICE: &str = "Prizes of one event gacha that use their own fanfare tier instead of the one their rarity gives\nEach prize is a kind, an ID and the tier\nKind 0 is a unit";

pub const FANFARE_LABELS: [&str; 13] = [
    "Event Gacha ID",
    "Prize 1 Kind",
    "Prize 1 ID",
    "Prize 1 Fanfare Tier",
    "Prize 2 Kind",
    "Prize 2 ID",
    "Prize 2 Fanfare Tier",
    "Prize 3 Kind",
    "Prize 3 ID",
    "Prize 3 Fanfare Tier",
    "Prize 4 Kind",
    "Prize 4 ID",
    "Prize 4 Fanfare Tier",
];

pub const SPOTLIGHT_NOTICE: &str = "A unit the Cat Guide points out as obtainable while this map is open";

pub const SPOTLIGHT_LABELS: [&str; 2] = ["Unit ID", "Map ID"];

pub const TIP_SETTING_NOTICE: &str = "When one title screen tip can be shown\nTips are picked from seven pools that the game tries in its own order, and a tip is in every pool whose cell is on\nPool 5 is tried until Empire of Cats progress reaches 100, pool 4 after that\nEvery other check is skipped at -1\nThe chapter cells are read by the Chapter Check, where 1 means that chapter must pass it and 0 means it must not\nThe two Treasure Festival checks only differ by a saved flag (the shipped tip that needs the flag is the returning players one)\nTreasure Gauge Full passes while the gauge is full and a chapter set to 0 is still missing superior treasures\nA stage is (Map ID * 100) + Stage\nThe two group cells look at the maps of the running half-energy event, where 1 means an open map of that group must be in it and 0 means none can be\nLabyrinth Event needs the Underground Labyrinth event to be in that state\nThe id cell is only checked for being set, and any value other than -1 makes the game read the map type cell as a whole Map ID and check that one map\nOnce Empire of Cats progress passes 99, the main menu also picks its top message from these tips using Main Menu Weight\nA Main Menu Picture Cut of -2 keeps the tip out of that pick, -1 shows the tip's text, and anything else draws that cut of the menu's message sheet instead";

pub const TIP_SETTING_LABELS: [&str; 29] = [
    "Tip ID",
    "In Pool 1 (j)",
    "In Pool 2 (w)",
    "In Pool 3 (s)",
    "In Pool 4 (l)",
    "In Pool 5 (i)",
    "In Pool 6 (d)",
    "In Pool 7 (lr)",
    "Chapter Check",
    "Empire Of Cats 1",
    "Empire Of Cats 2",
    "Empire Of Cats 3",
    "Into The Future 1",
    "Into The Future 2",
    "Into The Future 3",
    "Cats Of The Cosmos 1",
    "Cats Of The Cosmos 2",
    "Cats Of The Cosmos 3",
    "Half-Energy Group l Maps",
    "Half-Energy Group d Maps",
    "After Clearing Stage",
    "Before Clearing Stage",
    "Needs Open Map Of Type",
    "id",
    "Needs Open Map In Category",
    "Needs Open Ranking Stage",
    "Labyrinth Event",
    "Main Menu Picture Cut",
    "Main Menu Weight",
];

const FIRST_POOL: usize = 1;

const LAST_POOL: usize = 7;

const TIP_CHECK: usize = 8;

const TIP_LABYRINTH: usize = 26;

const TIP_PICTURE: usize = 27;

const TIP_WEIGHT: usize = 28;

const NO_MENU_DRAW: i32 = -2;

const CHAPTER_CHECKS: [Choice; 9] = [
    Choice::new(-1, "None"),
    Choice::new(0, "Superior Treasures Complete"),
    Choice::new(1, "Half-Energy Event On"),
    Choice::new(2, "Zombie Outbreak Present"),
    Choice::new(3, "Treasure Festival On, Flag Clear"),
    Choice::new(4, "Treasure Festival On, Flag Set"),
    Choice::new(5, "None (5)"),
    Choice::new(6, "None (6)"),
    Choice::new(7, "Treasure Gauge Full"),
];

const LABYRINTH_STATES: [Choice; 5] = [
    Choice::new(-1, "Any"),
    Choice::new(0, "Not Running"),
    Choice::new(1, "Event On"),
    Choice::new(2, "Results Being Tallied"),
    Choice::new(3, "Results Available"),
];

pub(super) fn tip_setting_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        FIRST_POOL..=LAST_POOL => Rule::Flag,
        TIP_CHECK => Rule::Choice(&CHAPTER_CHECKS),
        TIP_LABYRINTH => Rule::Choice(&LABYRINTH_STATES),
        TIP_PICTURE => Rule::Floor(NO_MENU_DRAW),
        TIP_WEIGHT => Rule::Floor(0),
        _ => Rule::Plain,
    }
}

pub const RATING_NOTICE: &str = "What this form is worth to the automatic lineup when one of its abilities counters the stage's enemies\nForms count from 1";

pub const RATING_LABELS: [&str; 3] = ["Unit ID", "Form", "Rating"];

const RATING: usize = 2;

pub(super) fn rating_rule(index: usize) -> Rule {
    match index {
        RATING => Rule::Plain,
        _ => Rule::Opaque,
    }
}

pub const BAND_NOTICE: &str = "One cost band the automatic lineup fills for a rank group\nIt takes units whose deploy cost is within the margin of the band's cost (written at chapter 1 prices)\nThe margin grows by 10 each time it comes up short\nOnly the first 10 bands of a group are read";

pub const RANK_GROUP_NOTICE: &str = "Which set of cost bands the automatic lineup uses\nGroups are tried in order, and the first one whose User Rank you have reached is used";

pub const BAND_LABELS: [&str; 4] = ["Rank Group", "Deploy Cost", "Cost Margin (%)", "Units Taken"];

pub const RANK_GROUP_LABELS: [&str; 2] = ["Rank Group", "User Rank"];

pub(super) fn rank_group_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Floor(0),
    }
}

pub const ABILITY_ICON_NOTICE: &str = "One ability the unit info panel, the Cat Guide and the unit filter can show\nIcon Cut is the picture drawn for the ability, and the battle panel's short list only shows kind 1\nFilter Icon Cut is the picture on the ability's button in the unit filter, and -1 leaves the ability out\nFilter Section is which of the five groups on that page the button sits in, counted from 0\nA row is known by its place in the file, not its row number\nThe table ends at the first row whose number is -1";

pub const TRAIT_ICON_NOTICE: &str = "One trait icon\nThe first ten rows are the traits the battle panel lists, sorted by Panel Order and drawn with Icon Cut\nThe two filter cells are the trait's button in the unit filter (the picture on it, and the button drawn while it is selected)\nAn unselected button is cut 1\nA row is known by its place in the file, not its row number\nThe table ends at the first row whose number is -1";

pub const ABILITY_ICON_LABELS: [&str; 7] = ["Row Number", "Order", "Kind", "Ability ID", "Filter Icon Cut", "Icon Cut", "Filter Section"];

pub const TRAIT_ICON_LABELS: [&str; 5] = ["Row Number", "Panel Order", "Icon Cut", "Filter Icon Cut", "Filter Selected Cut"];

pub(super) fn ability_icon_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Plain,
    }
}

pub(super) fn trait_icon_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        _ => Rule::Plain,
    }
}

pub const MENU_TEXT_NOTICE: &str = "When one main menu message can be shown\nThe chapter cells follow the chapter you last entered, and the message is hidden while that chapter's cell is off\nEvery map outside the three sagas counts as Other Maps, or Score Stages when it is scored\nThe two stage cells and the rank cell are skipped at 0\nA stage is (Map ID * 100) + Stage\nA Featured Event Map of -1 means none\nThe five unlock cells need that part of the game to be unlocked already\nCatfruit Storage also needs 48 cleared stages in Empire of Cats chapter 3\nAlso After Clearing Stage needs one more cleared stage, on a separate pick the menu makes\nRanking Map ID ties the message to that map's ranking event\nWhen that message is shown, the game saves the current time plus Hours Recorded When Shown for the event";

pub const MENU_TEXT_LABELS: [&str; 26] = [
    "Message ID",
    "Allowed In Restricted Pick",
    "In Empire Of Cats 1",
    "In Empire Of Cats 2",
    "In Empire Of Cats 3",
    "In Into The Future 1",
    "In Into The Future 2",
    "In Into The Future 3",
    "In Cats Of The Cosmos 1",
    "In Cats Of The Cosmos 2",
    "In Cats Of The Cosmos 3",
    "In Other Maps",
    "In Score Stages",
    "Shown Once",
    "After Clearing Stage",
    "Before Clearing Stage",
    "Needs Upgrade Menu Unlocked",
    "Needs Organize Menu Unlocked",
    "Needs Enemy Guide Unlocked",
    "Needs Gamatoto Unlocked",
    "Needs Catfruit Storage Unlocked",
    "User Rank At Least",
    "Featured Event Map",
    "Also After Clearing Stage",
    "Ranking Map ID",
    "Hours Recorded When Shown",
];

const MENU_FLAGS_END: usize = 13;

const MENU_STAGES_END: usize = 15;

const MENU_MARKS_START: usize = 16;

const MENU_MARKS_END: usize = 20;

const MENU_RANK: usize = 21;

const MENU_FEATURED: usize = 22;

const MENU_TAIL_START: usize = 23;

const MENU_TAIL_END: usize = 25;

pub(super) fn menu_text_rule(index: usize) -> Rule {
    match index {
        0 => Rule::Opaque,
        flag if flag <= MENU_FLAGS_END => Rule::Flag,
        stage if stage <= MENU_STAGES_END => Rule::Floor(0),
        MENU_MARKS_START..=MENU_MARKS_END => Rule::Flag,
        MENU_RANK => Rule::Floor(0),
        MENU_FEATURED => Rule::Floor(NO_MAP),
        MENU_TAIL_START..=MENU_TAIL_END => Rule::Floor(0),
        _ => Rule::Opaque,
    }
}
