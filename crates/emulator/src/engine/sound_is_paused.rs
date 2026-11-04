use super::SoundManager;

pub fn sound_is_paused(mgr: &SoundManager) -> bool {
    mgr.paused != 0
}
