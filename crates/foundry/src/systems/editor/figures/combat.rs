pub mod enemies;
pub mod cats;

fn shared_note(field: &str) -> Option<&'static str> {
    match field {
        "attack_cooldown" => Some(
            "The wait before the next attack, counted from the last hit\nIf the last hit never lands, the unit attacks again as soon as its attack animation ends",
        ),
        "time_until_attack_1" => Some(
            "The attack frame the first hit lands on\n0 means the unit never lands a hit\nFrame 0 and frames at or past the attack animation's length never land",
        ),
        "time_until_attack_2" => Some(
            "The attack frame the second hit lands on, 0 for one hit\nWhile this is 0 the third hit is ignored too\nA hit on the same frame as an earlier one never lands",
        ),
        "time_until_attack_3" => Some(
            "The attack frame the third hit lands on, 0 for two hits\nA hit on the same frame as an earlier one never lands",
        ),
        "spawn_animation_flag" => Some("Plays this unit's own entry animation when it spawns\nWithout it the entry file is never used"),
        "spawn_animation_type" => Some(
            "The shared entry effect it spawns with, -1 for none\nThe spawn lasts as long as this or its own entry, whichever is longer",
        ),
        "use_gudetama_soul" => Some("Plays this unit's own soul animation on its model when it dies\nWithout it the soul file is never used"),
        "soul_animation_type" => Some(
            "The shared soul effect it dies with, -1 for none, 0 for the usual one\nThe death lasts as long as this or its own soul, whichever is longer",
        ),
        _ => None,
    }
}
