use std::path::Path;

use crate::common::region;

pub(super) const TALK_PREFIX: &str = "talk_";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layout {
    Recipes,
    GamatotoHelpers,
    GamatotoUnlocks,
    GamatotoMessages,
    GodIntro,
    GodChatter,
    GodShortReply,
    GodPurchaseReply,
    GodMiracles,
    GodExplanation,
    StageLimits,
    FirstTips,
    WarningDialogs,
    QuitDialogs,
    MenuUnlocks,
    Tutorials,
    PopupMessages,
    CategoryNotes,
    CatfruitPopups,
    MenuMessages,
    DefeatTips,
    FirstDefeat,
    ChallengeUnlock,
    TitleTips,
    ChapterUnlocks,
    ShrineLevels,
    Fortunes,
    FortuneRewards,
    Talks,
    UnlockHints,
    HiddenHint,
    RankRewards,
    GamatotoCollabs,
    ShrineMembers,
    CollabSends,
    ClearGifts,
    UnseenEnemy,
    Vibrations,
    StageHints,
}

pub(super) const SHRINE: [Layout; 3] = [Layout::Fortunes, Layout::FortuneRewards, Layout::ShrineLevels];

pub(super) const INTERFACE: [Layout; 12] = [
    Layout::WarningDialogs,
    Layout::QuitDialogs,
    Layout::MenuUnlocks,
    Layout::Tutorials,
    Layout::PopupMessages,
    Layout::CategoryNotes,
    Layout::CatfruitPopups,
    Layout::MenuMessages,
    Layout::DefeatTips,
    Layout::FirstDefeat,
    Layout::ChallengeUnlock,
    Layout::TitleTips,
];

impl Layout {
    pub fn title(self) -> &'static str {
        match self {
            Layout::Recipes => "Recipe Text",
            Layout::GamatotoHelpers => "Helper Text",
            Layout::GamatotoUnlocks => "Area Unlock Text",
            Layout::GamatotoMessages => "Messages",
            Layout::GodIntro => "Intro",
            Layout::GodChatter => "Chatter",
            Layout::GodShortReply => "Short Reply",
            Layout::GodPurchaseReply => "Purchase Reply",
            Layout::GodMiracles => "Miracle Descriptions",
            Layout::GodExplanation => "Explanation",
            Layout::StageLimits => "Locked Stage Messages",
            Layout::FirstTips => "First Battle Tips",
            Layout::WarningDialogs => "Warning Dialogs",
            Layout::QuitDialogs => "Quit Dialogs",
            Layout::MenuUnlocks => "Menu Unlock Popups",
            Layout::Tutorials => "Tutorial Popups",
            Layout::PopupMessages => "Popup Messages",
            Layout::CategoryNotes => "Category Notes",
            Layout::CatfruitPopups => "Catfruit Popups",
            Layout::MenuMessages => "Cat Base Messages",
            Layout::DefeatTips => "Defeat Tips",
            Layout::FirstDefeat => "First Defeat Tip",
            Layout::ChallengeUnlock => "Challenge Mode Unlock",
            Layout::TitleTips => "Title Screen Tips",
            Layout::ChapterUnlocks => "Chapter Unlock Messages",
            Layout::ShrineLevels => "Shrine Level Messages",
            Layout::Fortunes => "Fortunes",
            Layout::FortuneRewards => "Fortune Rewards",
            Layout::Talks => "Event Talk",
            Layout::UnlockHints => "Unlock Hints",
            Layout::HiddenHint => "Hidden Unlock Hint",
            Layout::RankRewards => "User Rank Rewards",
            Layout::GamatotoCollabs => "Collab Members",
            Layout::ShrineMembers => "Shrine Member",
            Layout::CollabSends => "Collab Sends",
            Layout::ClearGifts => "Map Clear Gifts",
            Layout::UnseenEnemy => "Unseen Enemy Entry",
            Layout::Vibrations => "Battle Vibrations",
            Layout::StageHints => "Stage Hints",
        }
    }

    pub fn base(self) -> &'static str {
        match self {
            Layout::Recipes => "CastleRecipeDescriptions.csv",
            Layout::GamatotoHelpers => "GamatotoExpedition_Members_name.csv",
            Layout::GamatotoUnlocks => "GamatotoExpedition_Stage_Free.csv",
            Layout::GamatotoMessages => "GamatotoExpedition_Message.csv",
            Layout::GodIntro => "God1.csv",
            Layout::GodChatter => "God2.csv",
            Layout::GodShortReply => "God3.csv",
            Layout::GodPurchaseReply => "God4.csv",
            Layout::GodMiracles => "GodItemExplanation.csv",
            Layout::GodExplanation => "God_Explanation.csv",
            Layout::StageLimits => "MapStageLimitMessage.csv",
            Layout::FirstTips => "StageFirstMessage.csv",
            Layout::WarningDialogs => "Warning2.csv",
            Layout::QuitDialogs => "Option.csv",
            Layout::MenuUnlocks => "MainMenuPopUp.csv",
            Layout::Tutorials => "Tutorial.csv",
            Layout::PopupMessages => "PopUpMessage.csv",
            Layout::CategoryNotes => "Category_Explanation.csv",
            Layout::CatfruitPopups => "matatabi_Popup.csv",
            Layout::MenuMessages => "MainMenu.csv",
            Layout::DefeatTips => "Lose.csv",
            Layout::FirstDefeat => "FirstLose.csv",
            Layout::ChallengeUnlock => "ChallengeMode.csv",
            Layout::TitleTips => "TitleMessage.csv",
            Layout::ChapterUnlocks => "ED_Message.csv",
            Layout::ShrineLevels => "jinja_level.csv",
            Layout::Fortunes => "omikuji_text.csv",
            Layout::FortuneRewards => "omikuji_rewardtext.tsv",
            Layout::Talks => TALK_BASE,
            Layout::UnlockHints => "nyankoPictureBook.csv",
            Layout::HiddenHint => "nyankoPictureBookQuestion.csv",
            Layout::RankRewards => "user_info.tsv",
            Layout::GamatotoCollabs => "GamatotoCollabo.tsv",
            Layout::ShrineMembers => "jinja_member.tsv",
            Layout::CollabSends => "collaboSendData.tsv",
            Layout::ClearGifts => "StageClearBonus_setting.tsv",
            Layout::UnseenEnemy => "EnemyPictureBookQuestion.csv",
            Layout::Vibrations => "Vibration.csv",
            Layout::StageHints => "stage_hint_popup.csv",
        }
    }

    pub(super) fn headed(self) -> bool {
        matches!(self, Layout::ShrineMembers | Layout::ClearGifts | Layout::StageHints)
    }

    pub fn span(self) -> Option<(usize, usize)> {
        match self {
            Layout::WarningDialogs => Some((0, 8)),
            Layout::QuitDialogs | Layout::CategoryNotes => Some((0, 3)),
            Layout::MenuUnlocks | Layout::ChallengeUnlock => Some((0, 5)),
            Layout::Tutorials => Some((0, 15)),
            Layout::PopupMessages => Some((0, 10)),
            Layout::CatfruitPopups => Some((0, 6)),
            Layout::MenuMessages => Some((1, 6)),
            Layout::DefeatTips => Some((1, 5)),
            Layout::FirstDefeat => Some((0, 4)),
            Layout::TitleTips => Some((1, 2)),
            Layout::ChapterUnlocks => Some((0, 5)),
            Layout::ShrineLevels | Layout::Fortunes => Some((2, 1)),
            Layout::FortuneRewards | Layout::RankRewards => Some((1, 1)),
            Layout::Recipes
            | Layout::GamatotoHelpers
            | Layout::GamatotoUnlocks
            | Layout::GamatotoMessages
            | Layout::GodIntro
            | Layout::GodChatter
            | Layout::GodShortReply
            | Layout::GodPurchaseReply
            | Layout::GodMiracles
            | Layout::GodExplanation
            | Layout::StageLimits
            | Layout::FirstTips
            | Layout::UnlockHints
            | Layout::HiddenHint
            | Layout::GamatotoCollabs
            | Layout::ShrineMembers
            | Layout::CollabSends
            | Layout::ClearGifts
            | Layout::UnseenEnemy
            | Layout::Vibrations
            | Layout::StageHints
            | Layout::Talks => None,
        }
    }

    pub fn keyed(self) -> bool {
        matches!(
            self,
            Layout::Recipes
                | Layout::GamatotoHelpers
                | Layout::StageLimits
                | Layout::FortuneRewards
                | Layout::Talks
                | Layout::RankRewards
                | Layout::GamatotoCollabs
                | Layout::CollabSends
        )
    }

    pub fn comments(self) -> bool {
        matches!(self, Layout::GamatotoUnlocks | Layout::GamatotoMessages | Layout::HiddenHint)
    }

    pub fn named(self, key: u32, line: &str, delimiter: char) -> String {
        let cell = |at: usize| line.split(delimiter).nth(at).map(str::trim).filter(|cell| !cell.is_empty()).map(str::to_owned);

        match self {
            Layout::Recipes => cell(1).unwrap_or_else(|| key.to_string()),
            Layout::GamatotoHelpers => cell(3).map_or_else(|| format!("{HELPER_LABEL} {key}"), |name| format!("{key}: {name}")),
            Layout::GamatotoUnlocks => format!("{UNLOCK_LABEL} {key}"),
            Layout::GamatotoMessages => usize::try_from(key)
                .ok()
                .and_then(|at| MESSAGE_NAMES.get(at))
                .map_or_else(|| format!("{MESSAGE_LABEL} {key}"), |name| (*name).to_owned()),
            Layout::GodIntro => format!("{INTRO_LABEL} {}", key + 1),
            Layout::GodChatter => format!("{CHATTER_LABEL} {}", key + 1),
            Layout::GodMiracles => format!("{MIRACLE_LABEL} {}", key + 1),
            Layout::GodShortReply | Layout::GodPurchaseReply | Layout::GodExplanation => self.title().to_owned(),
            Layout::StageLimits => format!("{LIMIT_LABEL} {key}"),
            Layout::FirstTips => cell(0).map_or_else(|| format!("{TIP_LABEL} {}", key + 1), |title| format!("{TIP_LABEL} {}: {title}", key + 1)),
            Layout::FirstDefeat | Layout::ChallengeUnlock | Layout::UnlockHints | Layout::HiddenHint | Layout::ShrineMembers | Layout::UnseenEnemy => {
                self.title().to_owned()
            }
            Layout::GamatotoCollabs => format!("{COLLAB_LABEL} {key}"),
            Layout::CollabSends => format!("{SEND_LABEL} {key}"),
            Layout::Vibrations => {
                let event = cell(0).and_then(|id| id.parse::<usize>().ok()).and_then(|id| VIBRATION_EVENTS.get(id));
                let read = cell(1).is_some_and(|kind| VIBRATION_READ.contains(&kind.as_str()));

                match (event, read) {
                    (Some(event), true) => (*event).to_owned(),
                    (Some(event), false) => format!("{event} {VIBRATION_SKIPPED}"),
                    (None, _) => format!("{ENTRY_LABEL} {}", key + 1),
                }
            }
            Layout::StageHints => match (cell(0), cell(2)) {
                (Some(map), Some(stage)) => format!("{MAP_LABEL} {map}: {COLLAB_LABEL} {stage}"),
                _ => format!("{ENTRY_LABEL} {key}"),
            },
            Layout::ClearGifts => match cell(0).filter(|map| map != "0") {
                Some(map) => format!("{GIFT_LABEL} {key}: {MAP_LABEL} {map}"),
                None => format!("{GIFT_LABEL} {key}"),
            },
            Layout::WarningDialogs
            | Layout::QuitDialogs
            | Layout::MenuUnlocks
            | Layout::Tutorials
            | Layout::PopupMessages
            | Layout::CategoryNotes
            | Layout::CatfruitPopups
            | Layout::MenuMessages
            | Layout::DefeatTips
            | Layout::TitleTips
            | Layout::ChapterUnlocks
            | Layout::ShrineLevels
            | Layout::Fortunes
            | Layout::FortuneRewards
            | Layout::Talks
            | Layout::RankRewards => {
                let first = self.span().map_or(TALK_TEXT, |(first, _)| first);
                let shown = match self.keyed() {
                    true => key,
                    false => key + 1,
                };
                let lead = line.split(delimiter).skip(first).map(|cell| cell.replace(BREAK, " ")).map(|cell| cell.trim().to_owned()).find(|cell| !cell.is_empty() && cell != BLANK);

                match lead {
                    Some(text) if text.chars().count() > NAME_LENGTH => format!("{shown}: {}{NAME_CUT}", text.chars().take(NAME_LENGTH).collect::<String>().trim_end()),
                    Some(text) => format!("{shown}: {text}"),
                    None => format!("{ENTRY_LABEL} {shown}"),
                }
            }
        }
    }

    pub fn groups(self) -> &'static [Group] {
        match self {
            Layout::Recipes => &RECIPE_GROUPS,
            Layout::GamatotoHelpers => &HELPER_GROUPS,
            Layout::GamatotoUnlocks => &UNLOCK_GROUPS,
            Layout::GamatotoMessages => &MESSAGE_GROUPS,
            Layout::GodIntro => &GOD_INTRO_GROUPS,
            Layout::GodChatter => &GOD_CHATTER_GROUPS,
            Layout::GodShortReply | Layout::GodPurchaseReply => &GOD_REPLY_GROUPS,
            Layout::GodMiracles => &GOD_MIRACLE_GROUPS,
            Layout::GodExplanation => &GOD_EXPLANATION_GROUPS,
            Layout::StageLimits => &LIMIT_GROUPS,
            Layout::FirstTips => &TIP_GROUPS,
            Layout::UnlockHints => &HINT_GROUPS,
            Layout::HiddenHint => &HIDDEN_GROUPS,
            Layout::GamatotoCollabs => &COLLAB_GROUPS,
            Layout::ShrineMembers => &SHRINE_MEMBER_GROUPS,
            Layout::CollabSends => &SEND_GROUPS,
            Layout::ClearGifts => &GIFT_GROUPS,
            Layout::UnseenEnemy => &UNSEEN_GROUPS,
            Layout::Vibrations => &VIBRATION_GROUPS,
            Layout::StageHints => &STAGE_HINT_GROUPS,
            Layout::Talks => &TALK_GROUPS,
            Layout::WarningDialogs
            | Layout::QuitDialogs
            | Layout::MenuUnlocks
            | Layout::Tutorials
            | Layout::PopupMessages
            | Layout::CategoryNotes
            | Layout::CatfruitPopups
            | Layout::MenuMessages
            | Layout::DefeatTips
            | Layout::FirstDefeat
            | Layout::ChallengeUnlock
            | Layout::TitleTips
            | Layout::ChapterUnlocks
            | Layout::ShrineLevels
            | Layout::Fortunes
            | Layout::FortuneRewards
            | Layout::RankRewards => &[],
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Layout::Recipes => RECIPE_NOTE,
            Layout::GamatotoHelpers => HELPER_NOTE,
            Layout::GamatotoUnlocks => UNLOCK_NOTE,
            Layout::GamatotoMessages => MESSAGE_NOTE,
            Layout::GodIntro
            | Layout::GodChatter
            | Layout::GodShortReply
            | Layout::GodPurchaseReply
            | Layout::GodMiracles
            | Layout::GodExplanation => GOD_NOTE,
            Layout::StageLimits => LIMIT_NOTE,
            Layout::FirstTips => TIP_NOTE,
            Layout::UnlockHints => HINT_NOTE,
            Layout::HiddenHint => HIDDEN_NOTE,
            Layout::GamatotoCollabs | Layout::ShrineMembers => MEMBER_NOTE,
            Layout::CollabSends => SEND_NOTE,
            Layout::ClearGifts => GIFT_NOTE,
            Layout::UnseenEnemy => UNSEEN_NOTE,
            Layout::Vibrations => VIBRATION_NOTE,
            Layout::StageHints => STAGE_HINT_NOTE,
            Layout::Talks => TALK_NOTE,
            Layout::WarningDialogs
            | Layout::QuitDialogs
            | Layout::MenuUnlocks
            | Layout::Tutorials
            | Layout::PopupMessages
            | Layout::CategoryNotes
            | Layout::CatfruitPopups
            | Layout::MenuMessages
            | Layout::DefeatTips
            | Layout::FirstDefeat
            | Layout::ChallengeUnlock
            | Layout::TitleTips
            | Layout::ChapterUnlocks
            | Layout::ShrineLevels
            | Layout::Fortunes
            | Layout::FortuneRewards
            | Layout::RankRewards => SPAN_NOTE,
        }
    }

    pub fn width(self) -> usize {
        match self.span() {
            Some((first, count)) => first + count,
            None => self.groups().iter().flat_map(|group| group.cells.iter().map(|(at, _)| at + 1)).max().unwrap_or(1),
        }
    }
}

pub fn delimiter_of(file: &str) -> char {
    match Path::new(file).extension().and_then(|extension| extension.to_str()) {
        Some(TAB_EXTENSION) => TAB,
        _ => region::text_separator(file).char(),
    }
}

pub fn keyed(line: &str, delimiter: char) -> Option<u32> {
    line.split(delimiter).nth(KEY_CELL)?.trim().parse().ok()
}

pub const KEY_CELL: usize = 0;

const RECIPE_GROUPS: [Group; 4] = [
    Group { heading: "Recipe", cells: &[(1, "Name")] },
    Group { heading: "Cannon", cells: &[(6, "Name"), (9, "Effect Summary"), (5, "Description"), (4, "Recipe Blurb")] },
    Group {
        heading: "Foundation",
        cells: &[(7, "Name"), (14, "Effect Summary"), (12, "Description"), (10, "Part Text"), (2, "Recipe Blurb")],
    },
    Group { heading: "Style", cells: &[(8, "Name"), (15, "Effect Summary"), (13, "Description"), (11, "Part Text"), (3, "Recipe Blurb")] },
];

const RECIPE_NOTE: &str = "<br> starts a new line in the game\nThe Enhance Base entry only uses its recipe and cannon text";

const HELPER_GROUPS: [Group; 1] =
    [Group { heading: "Helper", cells: &[(3, "Name"), (4, "Title"), (5, "Description"), (6, "Effect Text")] }];

const HELPER_NOTE: &str = "\u{ff20} means an empty line in the game";

const UNLOCK_GROUPS: [Group; 1] = [Group { heading: "Unlock Text", cells: &[(0, "Line 1"), (1, "Line 2"), (2, "Line 3")] }];

const UNLOCK_NOTE: &str = "Shown on an area that is still locked\n\u{ff20} means an empty line";

const MESSAGE_GROUPS: [Group; 1] =
    [Group { heading: "Message", cells: &[(0, "Part 1"), (1, "Part 2"), (2, "Part 3"), (3, "Part 4"), (4, "Part 5")] }];

const MESSAGE_NOTE: &str = "The game reads up to 5 parts and stops at the first \u{ff20}";

const MESSAGE_NAMES: [&str; 34] = [
    "Log Loot Line",
    "Short Recon Course",
    "Normal Survey Course",
    "Full Expedition Course",
    "Course Select Popup",
    "Tutorial Popup 1",
    "Wardrobe Unlocked Popup",
    "Tutorial Popup 3",
    "Roll Call Unlocked Popup",
    "Tutorial Popup 5 (Retired)",
    "Tutorial Popup 6 (Retired)",
    "Tutorial Popup 7",
    "Catamin Used",
    "Expedition Success",
    "Expedition Great Success",
    "Level Up",
    "Forced Return",
    "Not Enough Catamins",
    "Log Item Found",
    "After Catamin",
    "Tutorial Popup 5-1",
    "Log Closing Line",
    "Area Unlocked",
    "Level Up With Outfit Change",
    "Helper Limit Reached",
    "Helper Dismissed",
    "Helper Joined",
    "Rarity Brackets",
    "Helper Count",
    "Results Count",
    "Ad Cat Food Icon",
    "Dismissal Confirmed",
    "Dismissal Confirmed, Assistants Full",
    "Legend Helper Unlocked",
];

const GOD_INTRO_GROUPS: [Group; 1] = [Group { heading: "Intro", cells: &PAIR_GROUP }];

const GOD_CHATTER_GROUPS: [Group; 1] = [Group { heading: "Chatter", cells: &PAIR_GROUP }];

const GOD_REPLY_GROUPS: [Group; 1] = [Group { heading: "Reply", cells: &PAIR_GROUP }];

const GOD_MIRACLE_GROUPS: [Group; 1] = [Group { heading: "Miracle", cells: &PAIR_GROUP }];

const GOD_EXPLANATION_GROUPS: [Group; 1] =
    [Group { heading: "Explanation", cells: &[(0, "Title"), (1, "Line 1"), (2, "Line 2"), (3, "Line 3")] }];

const GOD_NOTE: &str = "What the Cat God says\nA blank cell leaves that line empty";

const LIMIT_GROUPS: [Group; 1] = [Group { heading: "Message", cells: &[(1, "Line 1"), (2, "Line 2"), (3, "Line 3")] }];

const LIMIT_NOTE: &str = "Shown when you tap a locked stage\nThe ID is the map (or the map and stage) it belongs to";

const TIP_GROUPS: [Group; 1] = [Group { heading: "Tip", cells: &[(0, "Title"), (1, "Line 1"), (2, "Line 2"), (3, "Line 3")] }];

const TIP_NOTE: &str = "Shown during the very first battle\n\u{ff20} means an empty line";

const SPAN_NOTE: &str = "\u{ff20} means an empty line, or where the game stops reading the entry\n<br> starts a new line";

const ENTRY_LABEL: &str = "Entry";

const NAME_LENGTH: usize = 40;

const NAME_CUT: &str = "...";

const BREAK: &str = "<br>";

const BLANK: &str = "\u{ff20}";

pub(super) const TAB: char = '\t';

const HINT_GROUPS: [Group; 4] = [
    Group { heading: "Normal Form", cells: &FORM_LINES[0] },
    Group { heading: "Evolved Form", cells: &FORM_LINES[1] },
    Group { heading: "True Form", cells: &FORM_LINES[2] },
    Group { heading: "Ultra Form", cells: &FORM_LINES[3] },
];

const HINT_NOTE: &str = "How the Cat Guide says each form is obtained\n\u{ff20} means an empty line";

const HIDDEN_GROUPS: [Group; 1] = [Group { heading: "Hidden Hint", cells: &FORM_LINES[0] }];

const HIDDEN_NOTE: &str = "Shown instead of a cat's hint when the game hides where it comes from";

const COLLAB_GROUPS: [Group; 1] = [Group {
    heading: "Collab Member",
    cells: &[(1, "Member Animation"), (2, "Member Text Key Prefix"), (3, "Bubble Image"), (4, "Bubble Cut"), (5, "Scale")],
}];

const SHRINE_MEMBER_GROUPS: [Group; 1] = [Group {
    heading: "Shrine Member",
    cells: &[(0, "Member Animation"), (1, "Member Text Key Prefix"), (2, "Bubble Image"), (3, "Bubble Cut"), (4, "Scale")],
}];

const SEND_GROUPS: [Group; 1] = [Group {
    heading: "Collab Send",
    cells: &[(1, "Map ID"), (2, "Stage"), (3, "Collab Number"), (4, "Send Text Key"), (5, "Collaboration Text Key")],
}];

const MEMBER_NOTE: &str = "A guest member shown with Gamatoto\nThe text key is a prefix, the game reads its _name, _charatext and _popup lines from Localized Strings\nThe animation and the two bubble cells are file names";

const SEND_NOTE: &str = "Clearing this stage offers to open a partner app\nCollab Number picks the collabo_send_title_ and collabo_send_scheme_ lines in Localized Strings (the scheme is the link that opens)\nThe two text keys are lines in Localized Strings";

const GIFT_NOTE: &str = "A gift scene tied to a map\nIf more than one row qualifies, the lowest Priority wins\nNeeds Stage At Crown is (Map ID * 1000) + (Stage * 10) + Crown, and -1 means none\nThe row stops qualifying once its Unlock Popup is granted or its character is owned (-1 skips that check)\nGift Message Key is a line in Localized Strings";

const GIFT_GROUPS: [Group; 2] = [
    Group {
        heading: "Trigger",
        cells: &[(0, "Map ID"), (1, "Needs Stage At Crown"), (2, "Priority"), (3, "Unlock Popup ID"), (9, "Target Map ID")],
    },
    Group {
        heading: "Gift Scene",
        cells: &[(4, "Character (ID x10 + Kind)"), (5, "Character File Name"), (6, "Animation Loop 1"), (7, "Animation Loop 2"), (8, "Gift Message Key")],
    },
];

const UNSEEN_GROUPS: [Group; 1] = [Group {
    heading: "Unseen Enemy",
    cells: &[(0, "Name"), (1, "Line 1"), (2, "Line 2"), (3, "Line 3"), (4, "Line 4"), (5, "Line 5")],
}];

const VIBRATION_GROUPS: [Group; 1] = [Group {
    heading: "Vibration",
    cells: &[(0, "Event"), (1, "Row Kind"), (2, "Strength (0 To 1)"), (3, "Length (Milliseconds)"), (4, "Not Used On Android")],
}];

const VIBRATION_NOTE: &str = "How the phone vibrates for one battle event\nEvents 0 and 1 are a Cat or Enemy starting to die\nEvents 2 and 3 are a Cat or Enemy getting hit\nEvents 4 and 5 are the Cat Base or Enemy Base getting hit\nOnly rows with a kind of 1 or -1 are read\nA strength of 0 turns the event off\nThe length is kept between 20 and 30000";

const VIBRATION_EVENTS: [&str; 6] = ["Cat Dies", "Enemy Dies", "Cat Hit", "Enemy Hit", "Cat Base Hit", "Enemy Base Hit"];

const VIBRATION_READ: [&str; 2] = ["1", "-1"];

const VIBRATION_SKIPPED: &str = "(Not Read)";

const STAGE_HINT_GROUPS: [Group; 1] = [Group {
    heading: "Stage Hint",
    cells: &[(0, "Map ID"), (1, "Crown"), (2, "Stage"), (3, "Hint Text Key"), (4, "Memo")],
}];

const STAGE_HINT_NOTE: &str = "The hint offered on one stage, at one crown of its map\nCrowns and stages count from 0\nThe text key is a line in Localized Strings\nThe memo is never read";

const TALK_BASE: &str = "talk_000.tsv";

const TALK_TEXT: usize = 2;

const TALK_GROUPS: [Group; 1] = [Group {
    heading: "Talk Line",
    cells: &[(1, "Hours Before Next Line"), (TALK_TEXT, "Text"), (3, "Next Line"), (4, "Next Line For Second Answer")],
}];

const TALK_NOTE: &str = "One line of a Cat Base talk\nAfter a line with a wait, the talk pauses for that many hours\nEach answer leads to the line it names (a line with one target has no second answer)\nA target of -2 ends the talk and pays the reward\nThe first two rows of the file are not lines and can only be edited in Raw\nRow 1 holds the reward (Item ID in cell 5, count in cell 6), the line you must have reached to still get paid after the event ends (cell 7), and the talk's picture set number (cell 8)\nRow 2 holds the closing text for everyone else, and the Item ID and count they get (cells 5 and 6)";

const UNSEEN_NOTE: &str = "Shown in the Enemy Guide for an enemy you haven't met yet\n%s is the enemy's name";

const GIFT_LABEL: &str = "Gift";

const MAP_LABEL: &str = "Map";

const COLLAB_LABEL: &str = "Stage";

const SEND_LABEL: &str = "Send";

const LIMIT_LABEL: &str = "Message";

const TIP_LABEL: &str = "Tip";

const INTRO_LABEL: &str = "Intro";

const CHATTER_LABEL: &str = "Chatter";

pub(super) const MIRACLE_LABEL: &str = "Miracle";

pub(super) const MESSAGE_LABEL: &str = "Message";

const UNLOCK_LABEL: &str = "Unlock Text";

const HELPER_LABEL: &str = "Helper";

pub struct Group {
    pub heading: &'static str,
    pub cells: &'static [(usize, &'static str)],
}

const PAIR_GROUP: [(usize, &str); 2] = [(0, "Line 1"), (1, "Line 2")];

const FORM_LINES: [[(usize, &str); 3]; 4] = [
    [(0, "Line 1"), (1, "Line 2"), (2, "Line 3")],
    [(3, "Line 1"), (4, "Line 2"), (5, "Line 3")],
    [(6, "Line 1"), (7, "Line 2"), (8, "Line 3")],
    [(9, "Line 1"), (10, "Line 2"), (11, "Line 3")],
];

const TAB_EXTENSION: &str = "tsv";
