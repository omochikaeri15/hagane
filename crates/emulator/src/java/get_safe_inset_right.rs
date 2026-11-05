use super::{Activity, fit_scale};

pub fn get_safe_inset_right(activity: &Activity) -> i32 {
    if activity.sdk_int < Activity::CUTOUT_SDK {
        if activity.first_aspect >= 1.9 {
            return (88.0 * fit_scale(activity)) as i32;
        }

        return 0;
    }

    activity.display_cutout.map_or(0, |cutout| (cutout.right as f32 * fit_scale(activity)) as i32)
}
