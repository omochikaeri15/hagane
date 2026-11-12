use crate::common::terms::Terms;

const INTRO: [&str; 2] = [
    "The purpose of this agreement is to ensure that you, the User, are aware of the potential quirks regarding the use of a Provider.",
    "By accepting this agreement, you must have read, understood, and accepted the following facts about Providers:",
];
const MINING: &str = "Providers are incompatible with the \"Mining\" page, and will wipe its data.";
const MANIFEST: &str = "Providers wipe the import manifest, so the next import has to check every file again.";
const STALE: &str = "Providers may not keep recent or up to date game data.";
const CLOSING: &str = "You can accept the agreement by selecting the \"Agree\" button below. Selecting \"Disagree\" will close this pop-up.";

pub const TERMS: Terms = Terms { intro: INTRO, facts: &[MANIFEST, STALE], closing: CLOSING };

pub const MINING_TERMS: Terms = Terms { intro: INTRO, facts: &[MINING, MANIFEST, STALE], closing: CLOSING };
