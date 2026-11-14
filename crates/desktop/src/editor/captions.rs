use std::fs;
use std::path::{Path, PathBuf};

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Column};
use iced::{Element, Length, Padding, Size, Task};
use tracing::warn;

use codex::common;
use foundry::common::preview::{self, Stamp};
use foundry::common::region;
use foundry::domains::mods;
use foundry::domains::settings::EditorMode;
use foundry::Vfs;

use crate::app::theme;
use crate::common::feedback::{Slot, CONFIRM_LABEL, FAILURE_LABEL};
use crate::widget::{popup, smooth_scroll};

use super::{Mark, Reader};

const SIZE: Size = Size::new(460.0, 520.0);
const POPUP: popup::Spec = popup::Spec::new(popup::Kind::Captions, SIZE);

const BODY_PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const CARD_GAP: f32 = 6.0;
const CARD_PADDING: f32 = 8.0;
const LABEL_SIZE: f32 = 12.0;
const HEADING_SIZE: f32 = 14.0;
const NOTE_SIZE: f32 = 11.0;
const INPUT_SIZE: f32 = 13.0;
const INPUT_PADDING: f32 = 4.0;
const RAW_WIDTH: f32 = 280.0;
const LABEL_WIDTH: f32 = 120.0;
const BUTTON_SIZE: f32 = 13.0;
const BUTTON_PADDING: f32 = 8.0;
const SCROLL_GAP: f32 = 12.0;
const FRAME_GAP: f32 = 3.0;
const FRAME_INSET: f32 = theme::CARD_BORDER_WIDTH + FRAME_GAP;

const SYNC_LABEL: &str = "Sync With \"game\"";
const MISSING_LABEL: &str = "This entry has no line in the file";
const CELL_LABEL: &str = "Cell";
const KEY_CELL: usize = 0;
const COMMENT: &str = "//";

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

const PAIR_GROUP: [(usize, &str); 2] = [(0, "Line 1"), (1, "Line 2")];

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
const SPAN_LABEL: &str = "Line";
const ENTRY_LABEL: &str = "Entry";
const NAME_LENGTH: usize = 40;
const NAME_CUT: &str = "...";
const BREAK: &str = "<br>";
const BLANK: &str = "\u{ff20}";

const TAB_EXTENSION: &str = "tsv";
const TAB: char = '\t';

const FORM_LINES: [[(usize, &str); 3]; 4] = [
    [(0, "Line 1"), (1, "Line 2"), (2, "Line 3")],
    [(3, "Line 1"), (4, "Line 2"), (5, "Line 3")],
    [(6, "Line 1"), (7, "Line 2"), (8, "Line 3")],
    [(9, "Line 1"), (10, "Line 2"), (11, "Line 3")],
];

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
pub(super) const TALK_BASE: &str = "talk_000.tsv";
pub(super) const TALK_PREFIX: &str = "talk_";
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
const MIRACLE_LABEL: &str = "Miracle";
const MESSAGE_LABEL: &str = "Message";
const UNLOCK_LABEL: &str = "Unlock Text";
const HELPER_LABEL: &str = "Helper";

struct Group {
    heading: &'static str,
    cells: &'static [(usize, &'static str)],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Layout {
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
    pub(super) fn title(self) -> &'static str {
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

    pub(super) fn base(self) -> &'static str {
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

    fn span(self) -> Option<(usize, usize)> {
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

    pub(super) fn keyed(self) -> bool {
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

    fn comments(self) -> bool {
        matches!(self, Layout::GamatotoUnlocks | Layout::GamatotoMessages | Layout::HiddenHint)
    }

    pub(super) fn named(self, key: u32, line: &str, delimiter: char) -> String {
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

    fn groups(self) -> &'static [Group] {
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

    fn note(self) -> &'static str {
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

    fn width(self) -> usize {
        match self.span() {
            Some((first, count)) => first + count,
            None => self.groups().iter().flat_map(|group| group.cells.iter().map(|(at, _)| at + 1)).max().unwrap_or(1),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Plan {
    label: String,
    file: String,
    game: PathBuf,
    target_mod: String,
    values: EditorMode,
    layout: Layout,
    key: u32,
}

pub(super) struct Seed {
    pub(super) label: String,
    pub(super) file: String,
    pub(super) game: PathBuf,
    pub(super) target_mod: String,
    pub(super) values: EditorMode,
    pub(super) layout: Layout,
    pub(super) key: u32,
}

pub(super) fn plan(seed: Seed) -> Plan {
    let Seed { label, file, game, target_mod, values, layout, key } = seed;

    Plan { label, file, game, target_mod, values, layout, key }
}

impl Plan {
    fn held(&self, vfs: &Vfs) -> Option<PathBuf> {
        mods::find(vfs, &self.target_mod, Path::new(&self.file))
    }

    fn source(&self, vfs: &Vfs) -> PathBuf {
        self.held(vfs).unwrap_or_else(|| self.game.clone())
    }

    fn delimiter(&self) -> char {
        delimiter_of(&self.file)
    }
}

pub(super) fn delimiter_of(file: &str) -> char {
    match Path::new(file).extension().and_then(|extension| extension.to_str()) {
        Some(TAB_EXTENSION) => TAB,
        _ => region::text_separator(file).char(),
    }
}

pub(super) fn keyed(line: &str, delimiter: char) -> Option<u32> {
    line.split(delimiter).nth(KEY_CELL)?.trim().parse().ok()
}

fn located(lines: &[String], layout: Layout, key: u32, delimiter: char) -> Option<usize> {
    match layout.keyed() {
        true => lines.iter().position(|line| keyed(line, delimiter) == Some(key)),
        false => usize::try_from(key).ok().filter(|at| *at < lines.len()),
    }
}

fn entry(body: &str, layout: Layout, key: u32, delimiter: char) -> Option<String> {
    match layout.keyed() {
        true => body.lines().find(|line| keyed(line, delimiter) == Some(key)).map(str::to_owned),
        false => body.lines().nth(usize::try_from(key).ok()?).map(str::to_owned),
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Line {
    cells: Vec<String>,
    trailing: bool,
    tail: String,
}

impl Line {
    fn split(raw: &str, delimiter: char, comments: bool) -> Line {
        let (data, tail) = match raw.find(COMMENT).filter(|_| comments) {
            Some(at) => {
                let (data, comment) = raw.split_at(at);
                let kept = data.trim_end();

                (kept, format!("{}{comment}", &data[kept.len()..]))
            }
            None => (raw, String::new()),
        };

        let trailing = data.ends_with(delimiter);
        let body = data.strip_suffix(delimiter).unwrap_or(data);

        Line { cells: body.split(delimiter).map(str::to_owned).collect(), trailing, tail }
    }

    fn render(&self, delimiter: char) -> String {
        let mut line = self.cells.join(&delimiter.to_string());

        if self.trailing {
            line.push(delimiter);
        }

        line.push_str(&self.tail);

        line
    }

    fn cell(&self, at: usize) -> &str {
        self.cells.get(at).map_or("", String::as_str)
    }
}

struct Draft {
    plan: Plan,
    read_from: PathBuf,
    stamp: Stamp,
    lines: Vec<String>,
    row: Option<usize>,
    line: Line,
    failed: bool,
}

impl Draft {
    fn load(plan: Plan, vfs: &Vfs) -> Option<Draft> {
        let read_from = plan.source(vfs);

        let bytes = fs::read(&read_from)
            .inspect_err(|err| warn!(path = %read_from.display(), "Caption editor could not read the file: {}", err))
            .ok()?;

        let stamp = preview::stamp(&read_from)?;
        let lines: Vec<String> = common::scrub(&bytes).lines().map(str::to_owned).collect();

        let mut draft = Draft { plan, read_from, stamp, lines, row: None, line: Line::default(), failed: false };

        draft.index();

        Some(draft)
    }

    fn index(&mut self) {
        let delimiter = self.plan.delimiter();

        let layout = self.plan.layout;

        self.row = located(&self.lines, layout, self.plan.key, delimiter);
        self.line = self.row.and_then(|at| self.lines.get(at)).map_or_else(Line::default, |raw| Line::split(raw, delimiter, layout.comments()));
    }

    fn drifted(&self) -> bool {
        preview::stamp(&self.read_from) != Some(self.stamp)
    }

    fn write(&mut self, at: usize, typed: &str) -> bool {
        let delimiter = self.plan.delimiter();

        let layout = self.plan.layout;

        if (layout.keyed() && at == KEY_CELL) || at >= layout.width().max(self.line.cells.len()) || typed.contains(delimiter) || typed.contains('\n') {
            return false;
        }

        let Some(row) = self.row else {
            return false;
        };

        while self.line.cells.len() <= at {
            self.line.cells.push(String::new());
        }

        let Some(cell) = self.line.cells.get_mut(at) else {
            return false;
        };

        typed.clone_into(cell);

        let rendered = self.line.render(delimiter);

        let Some(slot) = self.lines.get_mut(row) else {
            return false;
        };

        if *slot == rendered {
            return false;
        }

        *slot = rendered;

        true
    }

    fn restore(&mut self, vfs: &Vfs) -> bool {
        let Ok(bytes) = fs::read(&self.plan.game) else {
            warn!(path = %self.plan.game.display(), "Caption editor could not read the vanilla file");

            return false;
        };

        let Some(original) = entry(&common::scrub(&bytes), self.plan.layout, self.plan.key, self.plan.delimiter()) else {
            warn!(path = %self.plan.game.display(), "Caption editor found no vanilla copy of this entry");

            return false;
        };

        let Some(slot) = self.row.and_then(|at| self.lines.get_mut(at)) else {
            return false;
        };

        *slot = original;
        self.index();

        self.persist(vfs)
    }

    fn destination(&self, vfs: &Vfs) -> Option<(PathBuf, Stamp)> {
        if self.read_from != self.plan.game {
            return Some((self.read_from.clone(), self.stamp));
        }

        let path = mods::ensure_as(vfs, &self.plan.target_mod, &self.plan.game, &self.plan.file)
            .inspect_err(|err| warn!(source = %self.plan.game.display(), "Caption editor could not stage the file: {}", err))
            .ok()?;

        if let Err(err) = vfs.create((self.plan.target_mod.as_str(), path.as_path())) {
            warn!(path = %path.display(), "Caption editor staged a file the index would not take: {}", err);
        }

        preview::stamp(&path).map(|stamp| (path, stamp))
    }

    fn persist(&mut self, vfs: &Vfs) -> bool {
        let Some((path, stamp)) = self.destination(vfs) else {
            self.failed = true;

            return false;
        };

        let mut body = self.lines.join("\n");
        body.push('\n');

        match preview::save(&path, body.as_bytes(), stamp) {
            Ok(fresh) => {
                self.read_from = path;
                self.stamp = fresh;
                self.failed = false;

                true
            }
            Err(err) => {
                warn!(path = %path.display(), "Caption editor could not write the file: {}", err);
                self.failed = true;

                false
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Popup(popup::Message),
    Typed(usize, String),
    Sync,
    ConfirmExpired,
    FailureExpired,
}

#[derive(Default)]
pub(super) struct State {
    draft: Option<Draft>,
    frame: popup::State,
    typed: Option<(usize, String)>,
    confirm: Slot<()>,
    failed: Slot<()>,
}

pub(super) fn kind() -> popup::Kind {
    POPUP.kind()
}

impl State {
    pub(super) fn begin(&mut self, plan: Plan, nudge: usize, vfs: &Vfs) {
        self.frame = popup::cascaded(nudge);
        self.typed = None;
        self.confirm.clear();
        self.failed.clear();
        self.draft = Draft::load(plan, vfs);
    }

    pub(super) fn drafting(&self) -> bool {
        self.draft.is_some()
    }

    pub(super) fn raised(&self) -> u64 {
        self.frame.raised()
    }

    pub(super) fn drifted(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::drifted)
    }

    pub(super) fn sync(&mut self, plan: Option<Plan>, vfs: &Vfs) {
        let Some(current) = self.draft.as_ref() else {
            return;
        };

        let Some(plan) = plan else {
            self.draft = None;

            return;
        };

        let moved = current.plan.held(vfs).is_some() != (current.read_from != current.plan.game);

        if current.plan != plan || current.drifted() || moved {
            self.typed = None;
            self.draft = Draft::load(plan, vfs);
        }
    }

    pub(super) fn update(&mut self, message: Message, vfs: &Vfs) -> Task<Message> {
        match message {
            Message::Popup(msg) => {
                if self.frame.update(msg, POPUP) {
                    self.draft = None;
                    self.typed = None;
                    self.confirm.clear();
                    self.failed.clear();
                }

                Task::none()
            }
            Message::Typed(at, typed) => {
                let Some(draft) = self.draft.as_mut() else {
                    return Task::none();
                };

                let changed = draft.write(at, &typed);

                self.typed = Some((at, typed));

                if changed && !draft.persist(vfs) {
                    return self.failed.set((), Message::FailureExpired);
                }

                Task::none()
            }
            Message::Sync => {
                if !self.confirm.take(&()) {
                    return self.confirm.set((), Message::ConfirmExpired);
                }

                self.typed = None;

                let restored = self.draft.as_mut().is_some_and(|draft| draft.restore(vfs));

                match restored {
                    true => Task::none(),
                    false => self.failed.set((), Message::FailureExpired),
                }
            }
            Message::ConfirmExpired => {
                self.confirm.expire();

                Task::none()
            }
            Message::FailureExpired => {
                self.failed.expire();

                Task::none()
            }
        }
    }

    fn typed_or(&self, at: usize, current: &str) -> String {
        match &self.typed {
            Some((held, typed)) if *held == at => typed.clone(),
            _ => current.to_owned(),
        }
    }

    pub(super) fn view(&self, window: Size) -> Option<Element<'_, Message>> {
        let draft = self.draft.as_ref()?;

        Some(self.frame.view(&draft.plan.label, POPUP, window, Message::Popup, move || self.body(draft), None))
    }

    fn body<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let listing: Element<'_, Message> = match (draft.row.is_some(), draft.plan.values) {
            (false, _) => container(theme::centered_text(MISSING_LABEL).size(LABEL_SIZE).style(text::danger))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into(),
            (true, EditorMode::Raw) => self.raw(draft),
            (true, EditorMode::Resolved) => self.resolved(draft),
        };

        let framed = container(
            smooth_scroll(scrollable(container(listing).padding(Padding::default().right(SCROLL_GAP))).width(Length::Fill).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(FRAME_INSET)
        .style(theme::card_container_outlined);

        let label = match (self.failed.is_set() || draft.failed, self.confirm.is_set()) {
            (true, _) => FAILURE_LABEL,
            (false, true) => CONFIRM_LABEL,
            (false, false) => SYNC_LABEL,
        };

        let sync = button(theme::button_label(label).size(BUTTON_SIZE))
            .width(Length::Fill)
            .padding(BUTTON_PADDING)
            .style(theme::danger_button)
            .on_press(Message::Sync);

        column![framed, sync]
            .spacing(GAP)
            .padding(BODY_PADDING)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn input<'a>(&'a self, at: usize, current: &str, width: Length) -> Element<'a, Message> {
        text_input("", &self.typed_or(at, current))
            .on_input(move |typed| Message::Typed(at, typed))
            .padding(INPUT_PADDING)
            .size(INPUT_SIZE)
            .width(width)
            .style(theme::rounded_input)
            .into()
    }

    fn raw<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let mut rows = Column::new().spacing(CARD_GAP).width(Length::Fill);

        for at in 0..draft.plan.layout.width().max(draft.line.cells.len()) {
            let current = draft.line.cell(at);
            let label = text(format!("{CELL_LABEL} {}", at + 1)).size(LABEL_SIZE).width(Length::Fill);

            let control: Element<'_, Message> = match draft.plan.layout.keyed() && at == KEY_CELL {
                true => text(current.to_owned()).size(INPUT_SIZE).width(Length::Fixed(RAW_WIDTH)).into(),
                false => self.input(at, current, Length::Fixed(RAW_WIDTH)),
            };

            rows = rows.push(row![label, control].spacing(GAP).align_y(Vertical::Center));
        }

        rows.into()
    }

    fn resolved<'a>(&'a self, draft: &'a Draft) -> Element<'a, Message> {
        let layout = draft.plan.layout;
        let mut cards = Column::new().spacing(CARD_GAP).width(Length::Fill);

        if let Some((first, count)) = layout.span() {
            let mut rows = column![text(layout.title()).size(HEADING_SIZE)].spacing(CARD_GAP);

            for at in first..first + count {
                rows = rows.push(
                    row![
                        text(format!("{SPAN_LABEL} {}", at - first + 1)).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)),
                        self.input(at, draft.line.cell(at), Length::Fill)
                    ]
                    .spacing(GAP)
                    .align_y(Vertical::Center),
                );
            }

            cards = cards.push(container(rows).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined));
        }

        for group in layout.groups() {
            let mut rows = column![text(group.heading).size(HEADING_SIZE)].spacing(CARD_GAP);

            for (at, label) in group.cells {
                rows = rows.push(
                    row![text(*label).size(LABEL_SIZE).width(Length::Fixed(LABEL_WIDTH)), self.input(*at, draft.line.cell(*at), Length::Fill)]
                        .spacing(GAP)
                        .align_y(Vertical::Center),
                );
            }

            cards = cards.push(container(rows).width(Length::Fill).padding(CARD_PADDING).style(theme::card_container_outlined));
        }

        cards.push(text(layout.note()).size(NOTE_SIZE).style(text::secondary)).into()
    }
}

pub(super) fn mark(plan: &Plan, vfs: &Vfs, reader: &mut Reader) -> Mark {
    let Some(held) = plan.held(vfs) else {
        return Mark::Same;
    };

    let delimiter = plan.delimiter();
    let ours = reader.text(&held).and_then(|body| entry(&body, plan.layout, plan.key, delimiter));
    let theirs = reader.text(&plan.game).and_then(|body| entry(&body, plan.layout, plan.key, delimiter));

    Mark::of(ours, theirs)
}

pub(super) fn reset(plan: Plan, vfs: &Vfs) -> bool {
    Draft::load(plan, vfs).is_some_and(|mut draft| draft.restore(vfs))
}

#[cfg(test)]
mod tests {
    use super::{keyed, Line};

    #[test]
    fn a_text_line_rewrites_one_cell_and_keeps_its_closing_divider() {
        let raw = "1|Slow Beam||Style text|";
        let mut line = Line::split(raw, '|', false);

        assert_eq!(keyed(raw, '|'), Some(1));
        assert_eq!(line.render('|'), raw);

        line.cells[2] = "Foundation text".to_owned();
        assert_eq!(line.render('|'), "1|Slow Beam|Foundation text|Style text|");
    }

    #[test]
    fn a_note_after_the_cells_rides_along_untouched() {
        let raw = "Tap here|to begin|\u{ff20}|\t\t\t//a note";
        let mut line = Line::split(raw, '|', true);

        assert_eq!(line.cells, ["Tap here", "to begin", "\u{ff20}"]);
        assert_eq!(line.render('|'), raw);

        line.cells[1] = "to start".to_owned();
        assert_eq!(line.render('|'), "Tap here|to start|\u{ff20}|\t\t\t//a note");
    }
}
