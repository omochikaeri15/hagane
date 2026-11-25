use codex::cat::unit::TalentGroup;

const HEADERS: [(i16, &str); 79] = [
    (1, "Weaken"),
    (2, "Freeze"),
    (3, "Slow"),
    (5, "Strong"),
    (6, "Tough Vs"),
    (7, "Massive Damage"),
    (8, "Knockback"),
    (10, "Attack Up"),
    (11, "Survives"),
    (13, "Critical"),
    (14, "Zombie Killer"),
    (15, "Barrier Breaker"),
    (16, "Extra Bounties"),
    (17, "Wave Attack"),
    (18, "Resist Weaken"),
    (19, "Resist Freeze"),
    (20, "Resist Slow"),
    (21, "Resist Knockback"),
    (22, "Resist Wave"),
    (25, "Cost Down"),
    (26, "Recover Speed Up"),
    (27, "Move Speed Up"),
    (29, "Curse Immunity"),
    (30, "Resist Curse"),
    (31, "Attack Buff"),
    (32, "HP Buff"),
    (33, "Target Red"),
    (34, "Target Floating"),
    (35, "Target Dark"),
    (36, "Target Metal"),
    (37, "Target Angel"),
    (38, "Target Alien"),
    (39, "Target Zombie"),
    (40, "Target Relic"),
    (41, "Target Traitless"),
    (44, "Immune to Weaken"),
    (45, "Immune to Freeze"),
    (46, "Immune to Slow"),
    (47, "Immune to Knockback"),
    (48, "Immune to Waves"),
    (49, "Warp Blocker"),
    (50, "Savage Blow"),
    (51, "Dodge Attack"),
    (52, "Resist Toxic"),
    (53, "Immune to Toxic"),
    (54, "Resist Surge"),
    (55, "Immune to Surge"),
    (56, "Surge Attack"),
    (57, "Target Aku"),
    (58, "Shield Piercing"),
    (59, "Soulstrike"),
    (60, "Curse"),
    (61, "Attack Frequency Up"),
    (62, "Mini-Wave"),
    (63, "Colossus Slayer"),
    (64, "Behemoth Slayer"),
    (65, "Mini-Surge"),
    (66, "Sage Slayer"),
    (67, "Explosion"),
    (68, "Counter-Surge"),
    (69, "Immune to Explosions"),
    (71, "HP Buff+"),
    (72, "Shortened Cooldown"),
    (300, "Slow Relic"),
    (301, "Weaken Relic"),
    (302, "Weaken Alien"),
    (303, "Slow Metal"),
    (304, "Knockback Zombie"),
    (305, "Knockback (Alien)"),
    (306, "Freeze Metal"),
    (307, "Dodge Attack [Zombie]"),
    (309, "Dodge Attack (All)"),
    (310, "Strong Against Relics"),
    (311, "Dodge Attack (Metal)"),
    (312, "Weaken (All)"),
    (313, "Freeze (All)"),
    (314, "Slow (All)"),
    (315, "Knockback (All)"),
    (316, "Curse (All)"),
];

pub(crate) fn name(group: &TalentGroup) -> Option<&'static str> {
    let image = group.name_image();

    HEADERS.iter().find(|(held, _)| *held == image).map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_header_is_listed_once() {
        for (at, (image, held)) in HEADERS.iter().enumerate() {
            assert!(!held.trim().is_empty(), "{image} has no name");
            assert!(HEADERS[..at].iter().all(|(held, _)| held != image), "{image} is listed twice");
        }
    }

    // A talent drawn under its own name picture goes by that picture, not by the ability's.
    #[test]
    fn a_name_follows_the_name_picture() {
        let plain = TalentGroup { ability_id: 3, name_id: -1, ..TalentGroup::default() };
        let named = TalentGroup { ability_id: 3, name_id: 300, ..TalentGroup::default() };
        let unknown = TalentGroup { ability_id: 250, name_id: -1, ..TalentGroup::default() };

        assert_eq!((name(&plain), name(&named), name(&unknown)), (Some("Slow"), Some("Slow Relic"), None));
    }
}
