pub(super) const EXP_TABLE: &str = "unitexp.csv";

pub(super) const FIRST_COST: u32 = 1;

pub const RIG_TITLES: [&str; 3] = ["Sprite Sheet", "Sprite Cuts", "Model"];

pub const ENTRY_SUFFIX: &str = "_entry";

pub const BANNER_TITLES: [&str; 4] = ["Normal Banner", "Evolved Banner", "True Banner", "Ultra Banner"];

pub const CAT_ICON_TITLE: &str = "Deploy Icon";

pub(super) const GACHA_IMAGE_TITLE: &str = "Gacha Image";

pub(super) const ICON_TITLE: &str = "Icon";

pub(super) const BANNER_TITLE: &str = "Banner";

pub const UPGRADE_TABLE: &str = "AbilityData.csv";

pub(super) const COMBO_PARAM_TABLE: &str = "NyancomboParam.tsv";

pub(super) const ORB_SLOT_TABLE: &str = "equipmentslot.csv";

pub(super) const DIFFICULTY_TABLE: &str = "difficulty_level.tsv";

pub(super) const EX_OPTION_TABLE: &str = "EX_option.csv";

pub(super) const CANNON_STATUS_TABLE: &str = "CC_AllParts_status.csv";

pub(super) const ORB_GRADE_TABLE: &str = "equipmentgrade.csv";

pub(super) const ORB_GRADE_LABEL: &str = "Grade";

pub(super) const ORB_GRADES: &str = "Orb Grades";

pub(super) const COMBO_EFFECT_LABEL: &str = "Effect";

pub(super) const UPGRADE_LABEL: &str = "Power Up";

pub const UPGRADE_NAMES: [&str; 10] = [
    "Cat Cannon Attack",
    "Cat Cannon Range",
    "Cat Cannon Charge",
    "Worker Cat Efficiency",
    "Worker Cat Wallet",
    "Cat Base Health",
    "Research",
    "Accounting",
    "Study",
    "Cat Energy",
];

pub const PRESET_PREFIX: &str = "Certification";

pub const EX_LOTTERY_TABLE: &str = "EX_lottery.csv";

pub(super) const SHAKE_TABLE: &str = "battleshake_setting.csv";

pub(super) const SHAKE_NAME: &str = "Shake";

pub(super) const RECIPE_TABLE: &str = "CastleRecipeUnlockData.csv";

pub(super) const RECIPE_NAME: &str = "Unlock";

pub(super) const DEVELOPMENT_COST_SETS: [(&str, &str); 3] = [("CastleRecipe_", "Cannon"), ("BaseRecipe_", "Foundation"), ("DecoRecipe_", "Style")];

pub(super) const DEVELOPMENT_COST_FOLDER: &str = "Development Costs";

pub(super) const DEVELOPMENT_COST_NAME: &str = "From Level";

pub(super) const DEVELOPMENT_TIER_TABLE: &str = "CastleRecipeUnlock.csv";

pub(super) const DEVELOPMENT_TIER_FOLDER: &str = "Development Tiers";

pub(super) const DEVELOPMENT_TIER_NAME: &str = "Tier";

pub(super) const DEVELOPMENT_PARTS: [&str; 3] = ["Cannon", "Foundation", "Style"];

pub(super) const MATERIAL_MIX_TABLE: &str = "CastleMixRecipe.csv";

pub(super) const MATERIAL_MIX_FOLDER: &str = "Material Mixing";

pub(super) const MATERIAL_MIX_NAME: &str = "Mix";

pub(super) const ENGINEER_CAP_TABLE: &str = "CastleCustomLimit.csv";

pub(super) const ENGINEER_CAP_NAME: &str = "Engineer Cap";

pub(super) const BAND_TABLE: &str = "autoset_organization.csv";

pub(super) const BAND_FOLDER: &str = "Lineup Cost Bands";

pub(super) const BAND_NAME: &str = "Cost Band";

pub(super) const RANK_GROUP_TABLE: &str = "autoset_organization_group.csv";

pub(super) const RANK_GROUP_FOLDER: &str = "Lineup Rank Groups";

pub(super) const RANK_GROUP_NAME: &str = "Rank Group";

pub(super) const ABILITY_ICON_TABLE: &str = "nyankoPictureBookData_EffectAbility.csv";

pub(super) const ABILITY_ICON_FOLDER: &str = "Ability Icons";

pub(super) const ABILITY_ICON_NAME: &str = "Ability Icon";

pub(super) const TRAIT_ICON_TABLE: &str = "nyankoPictureBookData_Attribute.csv";

pub(super) const TRAIT_ICON_FOLDER: &str = "Trait Icons";

pub(super) const TRAIT_ICON_NAME: &str = "Trait Icon";

pub(super) const MENU_TEXT_TABLE: &str = "MainMenuTextSetting.csv";

pub(super) const MENU_TEXT_FOLDER: &str = "Main Menu Message Settings";

pub(super) const MENU_TEXT_NAME: &str = "Message";

pub(super) const RATING_TABLE: &str = "autoset_chara_rating.csv";

pub(super) const RATING_NAME: &str = "Lineup Rating";

pub(super) const GUIDE_TABLE: &str = "nyankoPictureBookData.csv";

pub(super) const GUIDE_NAME: &str = "Cat Guide Entry";

pub(super) const UNIT_LIMIT_TABLE: &str = "unitlimit.csv";

pub(super) const UNIT_LIMIT_NAME: &str = "Level Cap Groups";

pub(super) const ENEMY_GUIDE_TABLE: &str = "enemy_dictionary_list.csv";

pub(super) const ENEMY_GUIDE_NAME: &str = "Enemy Guide Entry";

pub(super) const SHOP_TABLE: &str = "itemShopData.tsv";

pub(super) const SHOP_NAME: &str = "Slot";

pub(super) const ZOMBIE_TABLE: &str = "zombie_lottery.csv";

pub(super) const ZOMBIE_LINE: &str = "Line";

pub(super) const CPU_TABLE: &str = "ScatCPUsetting.csv";

pub(super) const CPU_LINE: &str = "Setting";

pub(super) const ORB_TRAIT_TABLE: &str = "equipment_attribute.csv";

pub(super) const ORB_TRAIT_LINE: &str = "Orb Trait";

pub(super) const ORB_TRAIT_FOLDER: &str = "Orb Trait Targets";

pub(super) const ALTAR_TABLE: &str = "DemonCastlelimit.csv";

pub(super) const GAMATOTO_FOLDER: &str = "Gamatoto";

pub(super) const STAMP_TABLE: &str = "StampData.csv";

pub(super) const STAMP_LINE: &str = "Line";

pub(super) const STAMP_FOLDER: &str = "Stamp Card Rewards";

pub(super) const STAMP_DAY: &str = "Day";

pub(super) const SLOT_TABLE: &str = "UnitSlot_UnlockData.csv";

pub(super) const SLOT_LINE: &str = "Line";

pub(super) const SLOT_FOLDER: &str = "Lineup Slot Unlocks";

pub(super) const SLOT_NAME: &str = "Slot";

pub(super) const CLUB_TABLE: &str = "NyankoClubData.csv";

pub(super) const CLUB_NAME: &str = "Reward";

pub(super) const CLUB_FOLDER: &str = "Officers' Club Rewards";

pub(super) const MISSION_TABLE: &str = "Mission_Data.csv";

pub(super) const MISSION_NAME: &str = "Mission";

pub(super) const MISSION_REWARDS: &str = "Mission Rewards";

pub(super) const MISSION_NAMES: &str = "Mission_Name.csv";

pub(super) const CONDITION_TABLE: &str = "Mission_Condition.csv";

pub(super) const MISSION_CONDITIONS: &str = "Mission Conditions";

pub(super) const MISSION_UNLOCK_TABLE: &str = "Mission_Unlock_Condition.csv";

pub(super) const MISSION_UNLOCK_NAME: &str = "Unlock";

pub(super) const MISSION_UNLOCKS: &str = "Mission Unlocks";

pub(super) const MISSION_GROUP_TABLE: &str = "Mission_Group.csv";

pub(super) const MISSION_GROUP_NAME: &str = "Group";

pub(super) const MISSION_GROUPS: &str = "Mission Groups";

pub(super) const MISSION_SHORTCUT_TABLE: &str = "Mission_Condition_Setting.csv";

pub(super) const MISSION_SHORTCUT_NAME: &str = "Shortcut";

pub(super) const MISSION_SHORTCUTS: &str = "Mission Shortcuts";

pub(super) const MISSION_DESIGNATION_TABLE: &str = "Mission_designation.csv";

pub(super) const MISSION_DESIGNATIONS: &str = "Mission Designations";

pub(super) const MONTHLY_MISSION_TABLE: &str = "Mission_Monthly.csv";

pub(super) const MONTHLY_MISSION_NAME: &str = "Monthly Set";

pub(super) const MONTHLY_MISSIONS: &str = "Monthly Missions";

pub(super) const SOUND_TABLE: &str = "Sound_setting.tsv";

pub(super) const SOUND_NAME: &str = "Sound";

pub(super) const SOUND_FOLDER: &str = "Sound Settings";

pub(super) const CHEST_TABLE: &str = "TreasureBox.tsv";

pub(super) const CHEST_NAME: &str = "Chest";

pub(super) const CHEST_FOLDER: &str = "Dojo Chests";

pub(super) const POPUP_TRIGGER_TABLE: &str = "unlockPopup.tsv";

pub(super) const POPUP_TRIGGER_NAME: &str = "Popup";

pub(super) const POPUP_TRIGGER_FOLDER: &str = "Unlock Popup Triggers";

pub(super) const COMBO_CATEGORY_TABLE: &str = "ComboFiltering.tsv";

pub(super) const COMBO_CATEGORY_NAME: &str = "Category";

pub(super) const COMBO_CATEGORY_FOLDER: &str = "Combo Filter Categories";

pub(super) const CASTLE_GROWTH_TABLE: &str = "CC_Castle_growth.csv";

pub(super) const CASTLE_BRACKET_NAME: &str = "Bracket";

pub(super) const CASTLE_GROWTH_FOLDER: &str = "Castle Health";

pub(super) const CASTLE_LEVEL_LABEL: &str = "Up To Level";

pub(super) const DEMON_CASTLE_TABLE: &str = "DemonCastledefine.csv";

pub(super) const MAP_SHEET_TABLE: &str = "MapData_Image.tsv";

pub(super) const MAP_SHEET_NAME: &str = "Map";

pub(super) const MAP_SHEET_FOLDER: &str = "Map Sheets";

pub(super) const DEMON_CASTLE_NAME: &str = "Castle";

pub(super) const DEMON_CASTLE_FOLDER: &str = "Aku Altar Castles";

pub(super) const CASTLE_NAME: &str = "Castle";

pub(super) const CASTLE_FOLDER: &str = "Castle Placement";

pub(super) const CASTLE_SETS: [(&str, &str, &str); 4] = [
    ("enemyCastleData0.csv", "Empire of Cats", "ec"),
    ("enemyCastleData1.csv", "Into the Future", "wc"),
    ("enemyCastleData2.csv", "Cats of the Cosmos", "sc"),
    ("enemyCastleDataLegend.csv", "Legend", "rc"),
];

pub(super) const CASTLE_IMAGE_TITLE: &str = "Castle Image";

pub(super) const BACKGROUND_IMAGE_TITLE: &str = "Background Image";

pub(super) const ITEM_ICON_TITLE: &str = "Item Icon";

pub(super) const IMAGE_TITLE: &str = "Image";

pub(super) const CASTLE_END: i32 = -999;

pub(super) const TIP_RULE_TABLE: &str = "LoseTextSetting.csv";

pub(super) const TIP_RULE_NAME: &str = "Rule";

pub(super) const TIP_RULE_FOLDER: &str = "Defeat Tip Rules";

pub(super) const QUEST_STAGE_NAME: &str = "Stage";

pub(super) const SLOT_LOTTERY_NAME: &str = "Clears";

pub(super) const QUEST_FOLDER: &str = "Legend Quest";

pub(super) const QUEST_STAGE_PREFIX: &str = "PlayDungeonD_";

pub(super) const SLOT_LOTTERY_PREFIX: &str = "RandomDungeon_";

pub(super) const QUEST_STAGE_FOLDER: &str = "Quest Stages";

pub(super) const SLOT_LOTTERY_FOLDER: &str = "Slot Lottery";

pub(super) const REALM_DRAW_NAME: &str = "Draw";

pub(super) const LABYRINTH_FLOOR_NAME: &str = "Floor";

pub(super) const LABYRINTH_FOLDER: &str = "Labyrinth";

pub(super) const LABYRINTH_PREFIX: &str = "PlayContentL_";

pub(super) const LABYRINTH_LOTTERY_TABLE: &str = "change_conditions.csv";

pub(super) const LABYRINTH_LOTTERY_FOLDER: &str = "Floor Lottery";

pub(super) const LABYRINTH_RULE_NAME: &str = "Rule";

pub(super) const REALM_ALTAR_NAME: &str = "Altar";

pub(super) const REALM_FOLDER: &str = "Aku Realm Lottery";

pub(super) const REALM_PREFIX: &str = "demon_lottery_DM";

pub(super) const GAMATOTO_LIMIT_TABLE: &str = "GamatotoExpedition_Limit.csv";

pub(super) const GAMATOTO_LIMIT_NAME: &str = "Expedition Limits";

pub(super) const GAMATOTO_UNLOCK_TABLE: &str = "GamatotoUnlockData.tsv";

pub(super) const GAMATOTO_UNLOCK_NAME: &str = "Unlock";

pub(super) const GAMATOTO_UNLOCKS: &str = "Unlocks";

pub(super) const GAMATOTO_DROP_TABLE: &str = "GamatotoExpedition_SpecialDropItem.csv";

pub(super) const GAMATOTO_DROP_NAME: &str = "Drop";

pub(super) const GAMATOTO_DROPS: &str = "Special Drops";

pub(super) const EXPEDITION_AREA_TABLES: [(&str, &str); 2] = [("GamatotoExpedition_Stage.csv", "Areas"), ("GamatotoExpedition_Stage_EVENT.csv", "Event Areas")];

pub(super) const EXPEDITION_AREA_NAME: &str = "Area";

pub(super) const GAMATOTO_LEVEL_TABLE: &str = "GamatotoExpedition.csv";

pub(super) const GAMATOTO_LEVEL_FOLDER: &str = "Levels";

pub(super) const GAMATOTO_LEVEL_NAME: &str = "Level";

pub(super) const PRIZE_SETS: [(&str, &str, &str); 2] = [("Slot_pattern_", "Slot Machines", "Slot"), ("Scratch_pattern_", "Scratch Cards", "Card")];

pub(super) const PRIZE_NAME: &str = "Prize";

pub(super) const REEL_PREFIX: &str = "slot_imagegroup_";

pub(super) const REEL_NAME: &str = "Symbol";

pub(super) const REEL_FOLDER: &str = "Reel Symbols";

pub(super) const PRIZE_ATTEMPT: &str = "Attempt";

pub(super) const SHRINE_OFFERING_TABLE: &str = "jinja_saisen.csv";

pub(super) const SHRINE_OFFERING_FOLDER: &str = "Offerings";

pub(super) const SHRINE_OFFERING_NAME: &str = "Offering";

pub(super) const SHRINE_EXTRA_TABLE: &str = "jinja_saisen_exdrop.csv";

pub(super) const SHRINE_EXTRA_FOLDER: &str = "Offering Bonuses";

pub(super) const SHRINE_EXTRA_NAME: &str = "Bonus";

pub(super) const FORTUNE_TABLE: &str = "omikuji_reward.csv";

pub(super) const FORTUNE_FOLDER: &str = "Fortune Odds";

pub(super) const FORTUNE_NAME: &str = "Fortune Rank";

pub(super) const ZONED_IN_TABLE: &str = "Gamatoto_Bonus.csv";

pub(super) const ZONED_IN_NAME: &str = "Zoned-In";

pub(super) const ZONED_IN_STEPS_NAME: &str = "Zoned-In Triggers";

pub(super) const FILTER_MAP_TABLE: &str = "FilterStage_Setting.csv";

pub(super) const FILTER_MAP_FOLDER: &str = "Stage Filter Maps";

pub(super) const FILTER_MAP_NAME: &str = "Filter Map";

pub(super) const EXCLUSION_TABLE: &str = "autoset_exclude_enemy.csv";

pub(super) const EXCLUSION_FOLDER: &str = "Lineup Exclusions";

pub(super) const EXCLUSION_NAME: &str = "Excluded Enemy";

pub(super) const ENIGMA_FOLDER: &str = "Enigma Stages";

pub(super) const ENIGMA_LOTTERY_TABLE: &str = "Hidden_lottery.csv";

pub(super) const ENIGMA_LOTTERY_FOLDER: &str = "Lotteries";

pub(super) const ENIGMA_LOTTERY_NAME: &str = "Lottery";

pub(super) const ENIGMA_RATE_TABLE: &str = "Hidden_rate.csv";

pub(super) const ENIGMA_RATE_FOLDER: &str = "Discovery Chances";

pub(super) const ENIGMA_DROP_TABLE: &str = "Hidden_drop.csv";

pub(super) const ENIGMA_DROP_FOLDER: &str = "Discovery Lotteries";

pub(super) const ENIGMA_LEVEL_NAME: &str = "Discovery Level";

pub(super) const ENIGMA_GROUP_TABLE: &str = "Hidden_group.csv";

pub(super) const ENIGMA_GROUP_FOLDER: &str = "Stage Discoveries";

pub(super) const ENIGMA_GROUP_NAME: &str = "Stage Discovery";

pub(super) const ENIGMA_MEDAL_TABLE: &str = "Discoverlv_growth.csv";

pub(super) const ENIGMA_MEDAL_FOLDER: &str = "Discovery Medals";

pub(super) const ENIGMA_MEDAL_NAME: &str = "Discovery Medal";

pub(super) const MAP_MARKER_PREFIX: &str = "mapCordData";

pub(super) const MAP_MARKER_FOLDER: &str = "World Map Markers";

pub(super) const MAP_MARKER_SET: &str = "World Map";

pub(super) const MAP_MARKER_NAME: &str = "Marker";

pub(super) const MAP_MARKER_ROWS: usize = 48;

pub(super) const ENIGMA_TIME_TABLE: &str = "Hidden_time.csv";

pub(super) const ENIGMA_TIME_FOLDER: &str = "Map Times";

pub(super) const ENIGMA_TIME_NAME: &str = "Enigma Map";

pub(super) const ENIGMA_RARITY_TABLE: &str = "Hidden_rarity.csv";

pub(super) const ENIGMA_RARITY_FOLDER: &str = "Rarities";

pub(super) const ENIGMA_RARITY_NAME: &str = "Rarity";

pub(super) const RECOMMEND_TABLE: &str = "beginner_recommend_powerup.csv";

pub(super) const LEVEL_UP_TABLE: &str = "Recommended_levelup.csv";

pub(super) const LEVEL_UP_FOLDER: &str = "Level-Up Offers";

pub(super) const LEVEL_UP_NAME: &str = "Event";

pub(super) const RECOMMEND_FOLDER: &str = "Recommended Upgrades";

pub(super) const RECOMMEND_NAME: &str = "Suggestion";

pub(super) const COUNTER_TABLE: &str = "autoset_effectability.csv";

pub(super) const COUNTER_FOLDER: &str = "Lineup Counters";

pub(super) const COUNTER_NAME: &str = "Counter";

pub(super) const ORB_CHOICE_TABLE: &str = "equipment_EffectAbility.csv";

pub(super) const ORB_CHOICE_FOLDER: &str = "Orb Filter Choices";

pub(super) const ORB_CHOICE_NAME: &str = "Choice";

pub(super) const ORB_OPTION_TABLE: &str = "equipment_option.csv";

pub(super) const ORB_OPTION_FOLDER: &str = "Orb Icon Options";

pub(super) const ORB_OPTION_NAME: &str = "Orb Ability";

pub(super) const REFUND_TABLE: &str = "LeadershipReturnMap.csv";

pub(super) const REFUND_FOLDER: &str = "Energy Refund Maps";

pub(super) const REFUND_NAME: &str = "Refund";

pub(super) const STAGE_SHORTCUT_TABLE: &str = "MapStage_Shortcut.csv";

pub(super) const STAGE_SHORTCUT_FOLDER: &str = "Stage Shortcuts";

pub(super) const STAGE_SHORTCUT_NAME: &str = "Shortcut";

pub(super) const FANFARE_TABLE: &str = "EventGatya_Setting.csv";

pub(super) const FANFARE_FOLDER: &str = "Fanfare Overrides";

pub(super) const FANFARE_NAME: &str = "Override";

pub(super) const SPOTLIGHT_TABLE: &str = "nyankoPictureBookData_CharaGet.csv";

pub(super) const SPOTLIGHT_FOLDER: &str = "Cat Guide Spotlights";

pub(super) const SPOTLIGHT_NAME: &str = "Spotlight";

pub(super) const TIP_SETTING_TABLE: &str = "TitleMessageSetting.csv";

pub(super) const TIP_SETTING_FOLDER: &str = "Title Tip Settings";

pub(super) const TIP_SETTING_NAME: &str = "Tip";

pub(super) const ALTAR_STAGE: &str = "Stage";

pub(super) const ALTAR_STAGE_SCALE: u32 = 100;

pub const EX_GROUP: &str = "Group";

pub const EX_MAP: &str = "Map";

pub const EX_STAGE: &str = "Stage";

pub const PRESET_EXTENSION: &str = ".preset";

pub(super) const ORB_GRADE: &str = "gradeID";

pub const REWARD_ITEM: &str = "DropItemID";

pub const NO_ITEM: i32 = -1;

pub const NO_ITEM_LABEL: &str = "Nothing";

pub(super) const ORB_EFFECT: &str = "content";

pub(super) const ORB_TRAIT: &str = "attribute";

pub const PRESET_CANNONS: [&str; 8] =
    ["Basic", "Slow Beam", "Iron Wall", "Thunderbolt", "Waterblast", "Holy Blast", "Breakerblast", "Curseblast"];

pub const ENEMY_ICON_TITLE: &str = "Enemy Icon";

pub const MAP_BANNER_TITLE: &str = "Map Banner";

pub const STAGE_BANNER_TITLE: &str = "Stage Banner";

pub(super) const FIRST_TALENT_TEXT: usize = 1;

pub(super) const GLOSSARY_FILES: [&str; 2] = ["nyankoPictureBook2.csv", "EnemyPictureBook2.csv"];

pub(super) const BACKGROUND_TABLE: &str = crate::domains::utilities::background::TABLE;

pub(super) const BACKGROUND_LABEL: &str = "Background";

pub const NO_EGGS: (i32, i32) = (-1, -1);
