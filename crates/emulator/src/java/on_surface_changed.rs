use super::Activity;

pub fn on_surface_changed(activity: &mut Activity, width: i32, height: i32) {
    if width == 0 || height == 0 {
        return;
    }

    if activity.first_width == 0 {
        activity.tablet = (width as f32) < height as f32 * 960.0 / 640.0;
        activity.first_width = width;
        activity.first_height = height;
        activity.first_aspect = width as f32 / height as f32;
        activity.offset_x = 0;
        activity.offset_y = 0;
        activity.fit_width = width;
        activity.fit_height = height;

        return;
    }

    let mut fit_width = activity.first_aspect as i32;
    let mut fit_height = 1;

    while fit_height <= height {
        let next = fit_height.wrapping_add(1);
        let scaled = next as f32 * activity.first_aspect;

        if width.wrapping_add(1) as f32 <= scaled {
            break;
        }

        fit_width = scaled as i32;
        fit_height = next;
    }

    activity.offset_x = width.wrapping_sub(fit_width) / 2;
    activity.offset_y = height.wrapping_sub(fit_height) / 2;
    activity.fit_width = fit_width;
    activity.fit_height = fit_height;
}
