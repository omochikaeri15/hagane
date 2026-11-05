mod activity;
mod fit_scale;
mod get_safe_inset_bottom;
mod get_safe_inset_left;
mod get_safe_inset_right;
mod get_safe_inset_top;
mod get_screen_window_ratio;
mod is_tablet;
mod on_surface_changed;

pub use activity::{Activity, DisplayCutout};
pub use fit_scale::fit_scale;
pub use get_safe_inset_bottom::get_safe_inset_bottom;
pub use get_safe_inset_left::get_safe_inset_left;
pub use get_safe_inset_right::get_safe_inset_right;
pub use get_safe_inset_top::get_safe_inset_top;
pub use get_screen_window_ratio::get_screen_window_ratio;
pub use is_tablet::is_tablet;
pub use on_surface_changed::on_surface_changed;
