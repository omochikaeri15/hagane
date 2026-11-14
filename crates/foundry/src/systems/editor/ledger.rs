use crate::domains::sandbox::base;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Assembly {
    Cannon,
    Foundation,
    Style,
}

pub(super) const ASSEMBLIES: [Assembly; 3] = [Assembly::Cannon, Assembly::Foundation, Assembly::Style];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ledger {
    StageOption,
    FixedFormation,
    StageDraws,
    Checkpoints,
    StageLimits,
    ExDraws,
    CpuLocks,
    Growth(Assembly),
}

impl Ledger {
    pub fn title(self) -> &'static str {
        match self {
            Ledger::StageOption => "Restrictions",
            Ledger::FixedFormation => "Lineup Assignments",
            Ledger::StageDraws => "Stage Draws",
            Ledger::Checkpoints => "Stage Checkpoints",
            Ledger::StageLimits => "Stage Unlocks",
            Ledger::ExDraws => "EX Stage Draws",
            Ledger::CpuLocks => "Super CPU Lock",
            Ledger::Growth(Assembly::Cannon) => "Cannon Growth",
            Ledger::Growth(Assembly::Foundation) => "Foundation Growth",
            Ledger::Growth(Assembly::Style) => "Style Growth",
        }
    }

    pub fn folder(self) -> &'static str {
        match self {
            Ledger::Growth(Assembly::Cannon) => "Cannons",
            Ledger::Growth(Assembly::Foundation) => "Foundations",
            Ledger::Growth(Assembly::Style) => "Styles",
            Ledger::StageOption
            | Ledger::FixedFormation
            | Ledger::StageDraws
            | Ledger::Checkpoints
            | Ledger::StageLimits
            | Ledger::ExDraws
            | Ledger::CpuLocks => self.title(),
        }
    }

    pub fn base(self) -> &'static str {
        match self {
            Ledger::StageOption => "Stage_option.csv",
            Ledger::FixedFormation => "fixed_formation.csv",
            Ledger::StageDraws => "stage_conditions.csv",
            Ledger::Checkpoints => "Stageskip_setting.csv",
            Ledger::StageLimits => "PlayStageLimit.csv",
            Ledger::ExDraws => "EX_group.csv",
            Ledger::CpuLocks => "LockSkipData.csv",
            Ledger::Growth(Assembly::Cannon) => base::CANNON_GROWTH,
            Ledger::Growth(Assembly::Foundation) => base::FOUNDATION_GROWTH,
            Ledger::Growth(Assembly::Style) => base::STYLE_GROWTH,
        }
    }

    pub fn width(self) -> usize {
        match self {
            Ledger::StageOption => STAGE_OPTION_WIDTH,
            Ledger::FixedFormation => FORMATION_WIDTH,
            Ledger::StageDraws => DRAW_WIDTH,
            Ledger::Checkpoints => CHECKPOINT_WIDTH,
            Ledger::StageLimits => LIMIT_WIDTH,
            Ledger::ExDraws => EX_WIDTH,
            Ledger::CpuLocks => LOCK_WIDTH,
            Ledger::Growth(_) => GROWTH_WIDTH,
        }
    }

    pub fn field(self, column: usize) -> &'static str {
        let fields: &[&'static str] = match self {
            Ledger::StageOption => &STAGE_OPTION_FIELDS,
            Ledger::FixedFormation => &FORMATION_FIELDS,
            Ledger::StageDraws => &DRAW_FIELDS,
            Ledger::Checkpoints => &CHECKPOINT_FIELDS,
            Ledger::StageLimits => &LIMIT_FIELDS,
            Ledger::ExDraws => &EX_FIELDS,
            Ledger::CpuLocks => &LOCK_FIELDS,
            Ledger::Growth(_) => &GROWTH_FIELDS,
        };

        fields.get(column).copied().unwrap_or_default()
    }

    pub fn text(self, column: usize) -> bool {
        matches!(
            (self, column),
            (Ledger::FixedFormation, FORMATION_PRESET | FORMATION_MEMO) | (Ledger::Checkpoints, CHECKPOINT_MESSAGE)
        )
    }

    pub fn blank(self, column: usize) -> String {
        match self.text(column) || (self == Ledger::ExDraws && column >= EX_FIRST_PAIR) {
            true => String::new(),
            false => 0.to_string(),
        }
    }

    pub fn key_column(self) -> usize {
        match self {
            Ledger::ExDraws => EX_MAP,
            Ledger::CpuLocks => LOCK_MAP,
            _ => KEY,
        }
    }

    pub fn trims(self) -> bool {
        self == Ledger::ExDraws
    }

    pub fn noun(self) -> &'static str {
        match self {
            Ledger::StageOption => "Restriction",
            Ledger::FixedFormation => "Assignment",
            Ledger::StageDraws => "Draw",
            Ledger::Checkpoints => "Checkpoint",
            Ledger::StageLimits => "Unlock",
            Ledger::ExDraws => "EX Draw",
            Ledger::CpuLocks => "Lock",
            Ledger::Growth(_) => "Bracket",
        }
    }

    pub fn add_label(self) -> &'static str {
        match self {
            Ledger::StageOption => "Add Restriction",
            Ledger::FixedFormation => "Add Assignment",
            Ledger::StageDraws => "Add Draw",
            Ledger::Checkpoints => "Add Checkpoint",
            Ledger::StageLimits => "Add Unlock",
            Ledger::ExDraws => "Add EX Draw",
            Ledger::CpuLocks => "Add Lock",
            Ledger::Growth(_) => "Add Bracket",
        }
    }

    pub fn empty_label(self) -> &'static str {
        match self {
            Ledger::StageOption => "No restrictions on this map",
            Ledger::FixedFormation => "No stage on this map has a fixed lineup",
            Ledger::StageDraws => "No stage on this map is drawn from a pool",
            Ledger::Checkpoints => "This map has no checkpoints",
            Ledger::StageLimits => "No stage on this map waits on a condition",
            Ledger::ExDraws => "No stage on this map draws an EX stage",
            Ledger::CpuLocks => "The Super CPU may run this map",
            Ledger::Growth(_) => "This part has no growth brackets",
        }
    }

    pub fn seed(self, key: u32, presets: &[String]) -> Vec<String> {
        match self {
            Ledger::StageOption => {
                let mut cells = vec![0.to_string(); STAGE_OPTION_WIDTH];

                cells[KEY] = key.to_string();
                cells[CROWN] = EVERY.to_string();
                cells[STAGE] = EVERY.to_string();
                cells.push(String::new());

                cells
            }
            Ledger::FixedFormation => {
                let mut cells: Vec<String> = (0..FORMATION_WIDTH).map(|column| self.blank(column)).collect();

                cells[KEY] = key.to_string();

                if let Some(first) = presets.first() {
                    first.clone_into(&mut cells[FORMATION_PRESET]);
                }

                cells
            }
            Ledger::StageDraws => {
                let mut cells = vec![0.to_string(); DRAW_WIDTH];

                cells[KEY] = key.to_string();
                cells.push(String::new());

                cells
            }
            Ledger::CpuLocks => vec![0.to_string(), key.to_string()],
            Ledger::ExDraws => vec![
                EX_RATE_DEFAULT.to_owned(),
                key.to_string(),
                0.to_string(),
                0.to_string(),
                EX_RATE_DEFAULT.to_owned(),
            ],
            Ledger::StageLimits => {
                let mut cells = vec![0.to_string(); LIMIT_WIDTH];

                cells[KEY] = key.to_string();

                cells
            }
            Ledger::Checkpoints => {
                let mut cells: Vec<String> = (0..CHECKPOINT_WIDTH).map(|column| self.blank(column)).collect();

                cells[KEY] = key.to_string();

                cells
            }
            Ledger::Growth(_) => {
                let mut cells = vec![0.to_string(); GROWTH_WIDTH];

                cells[KEY] = key.to_string();

                cells
            }
        }
    }
}

pub const EVERY: i32 = -1;

const KEY: usize = 0;

pub const CROWN: usize = 1;

pub const STAGE: usize = 2;

const STAGE_OPTION_WIDTH: usize = 9;

const FORMATION_WIDTH: usize = 5;

pub const FORMATION_PRESET: usize = 3;

const FORMATION_FIELDS: [&str; FORMATION_WIDTH] = ["MapID", "Level", "StageNo", "PresetFileName", "MEMO"];

const DRAW_WIDTH: usize = 5;

const DRAW_FIELDS: [&str; DRAW_WIDTH] = ["map_id", "first_stage", "last_stage", "first_layout", "last_layout"];

const CHECKPOINT_WIDTH: usize = 4;

const CHECKPOINT_FIELDS: [&str; CHECKPOINT_WIDTH] = ["map_id", "stage", "skip_limit", "message_label"];

const LIMIT_WIDTH: usize = 3;

const LIMIT_FIELDS: [&str; LIMIT_WIDTH] = ["map_id", "stage", "condition"];

const EX_WIDTH: usize = 9;

pub(super) const EX_MAP: usize = 1;

pub const EX_FIRST_PAIR: usize = 3;

const EX_RATE_DEFAULT: &str = "100";

const EX_FIELDS: [&str; EX_WIDTH] = ["rate", "map_id", "stage", "group_1", "chance_1", "group_2", "chance_2", "group_3", "chance_3"];

const LOCK_WIDTH: usize = 2;

const LOCK_MAP: usize = 1;

const LOCK_FIELDS: [&str; LOCK_WIDTH] = ["message_type", "map_id"];

const GROWTH_WIDTH: usize = 6;

const GROWTH_FIELDS: [&str; GROWTH_WIDTH] = ["id", "type", "lv2", "value1", "value2", "easing"];

const STAGE_OPTION_FIELDS: [&str; STAGE_OPTION_WIDTH] = [
    "map_id",
    "target_crowns",
    "target_stage",
    "rarity_mask",
    "deploy_limit",
    "allowed_rows",
    "min_cost",
    "max_cost",
    "charagroup_id",
];

pub const FORMATION_MEMO: usize = 4;

pub const CHECKPOINT_MESSAGE: usize = 3;
