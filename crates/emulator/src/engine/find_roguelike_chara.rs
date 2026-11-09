use super::RoguelikeProgress;

pub fn find_roguelike_chara(progress: &RoguelikeProgress, unit_id: i32) -> Option<usize> {
    let mut index = 0;

    while index != progress.roster.len() {
        if progress.roster[index].unit_id == unit_id {
            break;
        }

        index += 1;
    }

    if index != progress.roster.len() { Some(index) } else { None }
}
