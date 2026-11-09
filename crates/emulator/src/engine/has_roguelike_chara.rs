use super::RoguelikeProgress;

pub fn has_roguelike_chara(progress: &RoguelikeProgress, unit_id: i32) -> bool {
    for chara in &progress.roster {
        if chara.unit_id == unit_id {
            return true;
        }
    }

    false
}
