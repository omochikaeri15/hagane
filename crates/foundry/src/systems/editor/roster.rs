pub(super) const FOLDER: &str = "Gacha";

pub(super) const SET_NAME: &str = "Set";

pub(super) const CAPSULES: [(&str, &str); 3] = [("R", "Rare Capsules"), ("N", "Normal Capsules"), ("E", "Event Capsules")];

pub(super) const TIERS: [Tier; 3] = [Tier::Units, Tier::Second, Tier::Third];

pub(super) const PREFIX: &str = "GatyaDataSet";

pub(super) const CHANCE_TABLE: &str = "GatyaData_Option_ChanceAnimation.tsv";

pub(super) const CHANCE_NAME: &str = "Rule";

pub(super) const CHANCE_FOLDER: &str = "Chance Animations";

pub(super) fn chance_name(capsule: Option<i64>, series: Option<i64>) -> Option<String> {
    let heading = CAPSULES.get(usize::try_from(capsule?).ok()?).map(|(_, heading)| *heading)?;

    let series = match series? {
        -1 => ANY_SERIES.to_owned(),
        held => format!("{SERIES_LABEL} {held}"),
    };

    Some(format!("{heading}, {series}"))
}

pub(super) fn options_file(capsule: &str) -> String {
    format!("{OPTIONS}{capsule}.{OPTIONS_EXTENSION}")
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tier {
    Units,
    Second,
    Third,
}

impl Tier {
    pub fn title(self) -> &'static str {
        match self {
            Tier::Units => "Unit Roster",
            Tier::Second => "Second Roster",
            Tier::Third => "Third Roster",
        }
    }

    pub fn number(self) -> usize {
        match self {
            Tier::Units => 1,
            Tier::Second => 2,
            Tier::Third => 3,
        }
    }

    pub fn file(self, capsule: &str) -> String {
        format!("{PREFIX}{capsule}{}.{EXTENSION}", self.number())
    }

    pub fn unique(self) -> bool {
        self == Tier::Units
    }

    pub fn note(self) -> &'static str {
        match self {
            Tier::Units => UNITS_NOTE,
            Tier::Second | Tier::Third => ENTRIES_NOTE,
        }
    }
}

const EXTENSION: &str = "csv";

const OPTIONS: &str = "GatyaData_Option_Set";

const OPTIONS_EXTENSION: &str = "tsv";

const ANY_SERIES: &str = "Any Series";

const SERIES_LABEL: &str = "Series";

const UNITS_NOTE: &str = "The cats this capsule set can roll";

const ENTRIES_NOTE: &str = "The game reads this list for the set, but nothing it rolls is known to come from it";
