use super::{Activity, fit_scale};

pub fn get_safe_inset_bottom(activity: &Activity) -> i32 {
    if activity.sdk_int < Activity::CUTOUT_SDK {
        return 0;
    }

    activity.display_cutout.map_or(0, |cutout| (cutout.bottom as f32 * fit_scale(activity)) as i32)
}
