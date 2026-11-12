use crate::common::terms::Terms;

pub const TERMS: Terms = Terms {
    intro: [
        "The purpose of this agreement is to ensure that you, the User, are aware of the potential quirks regarding Sandbox.",
        "By accepting this agreement, you must have read, understood, and accepted the following facts about Sandbox:",
    ],
    facts: &[
        "Sandbox may have known but subtle inaccuracies, mechanical bugs, or logical quirks when compared to the real game.",
        "Sandbox may not be updated to support features and/or mechanics from the latest game version(s) within a timely manner.",
        "Sandbox is representative of the vanilla game logic and its capabilities only, with no custom functionality added or planned.",
        "Sandbox is not an avenue that replaces the appeal and purpose of the game in any capacity, whether that be vanilla or modded.",
        "Sandbox does not guarantee backwards compatibility with older game versions and replays created in said versions will drift.",
    ],
    closing: "You can accept the agreement by selecting the \"Agree\" button below. Selecting \"Disagree\" will return you to the Home page.",
};
