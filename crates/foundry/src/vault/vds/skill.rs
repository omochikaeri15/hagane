use codex::graphics::rig::Animation;

use crate::Vfs;

use super::Slot;

const WAVE: &str = "skill_wave_attack.maanim";
const MINI_WAVE: &str = "skill_smallwave_attack.maanim";

#[derive(Default)]
pub struct SkillStore {
    waves: Slot<[i32; 2]>,
}

impl SkillStore {
    pub fn wave_frames(&self, vfs: &Vfs) -> [i32; 2] {
        *super::cached(&self.waves, || [WAVE, MINI_WAVE].map(|file| super::parsed(vfs, file, |bytes| Animation::scan_length(bytes).ok_or(())).unwrap_or(0)))
    }

    pub(super) fn evict(&self, filename: &str) -> bool {
        let held = filename == WAVE || filename == MINI_WAVE;

        if held {
            super::reset(&self.waves);
        }

        held
    }

    pub(super) fn clear(&self) {
        super::reset(&self.waves);
    }
}
