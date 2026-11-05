use super::Activity;

pub fn fit_scale(activity: &Activity) -> f32 {
    activity.fit_width as f32 / activity.first_width as f32
}
