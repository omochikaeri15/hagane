use super::Activity;

pub fn get_screen_window_ratio(activity: &Activity) -> f32 {
    if !activity.in_multi_window {
        return 1.0;
    }

    let width = activity.fit_width.wrapping_add(activity.offset_x.wrapping_mul(2)) as f32;
    let height = activity.fit_height.wrapping_add(activity.offset_y.wrapping_mul(2)) as f32;
    let (display_width, display_height) = activity.display_size;

    width * height / display_width.wrapping_mul(display_height) as f32
}
