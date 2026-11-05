use std::time::{SystemTime, UNIX_EPOCH};

use std::cell::Cell;
use std::rc::Rc;

use crate::engine::Platform;
use crate::java::{Activity, on_surface_changed};

pub struct DeviceProfile {
    pub tablet: Cell<bool>,
    pub side_inset: Cell<i32>,
    pub top_inset: Cell<i32>,
    pub bottom_inset: Cell<i32>,
    pub window_ratio: Cell<f32>,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self { tablet: Cell::new(false), side_inset: Cell::new(0), top_inset: Cell::new(0), bottom_inset: Cell::new(0), window_ratio: Cell::new(1.0) }
    }
}

pub fn phone_activity(width: i32, height: i32) -> Activity {
    let mut activity = Activity { sdk_int: Activity::CUTOUT_SDK - 1, ..Activity::default() };

    on_surface_changed(&mut activity, width, height);

    activity
}

#[derive(Default)]
pub struct InertPlatform {
    pub profile: Rc<DeviceProfile>,
}

impl Platform for InertPlatform {
    fn set_keep_awake(&mut self, _awake: bool) {}

    fn is_tablet(&mut self) -> bool {
        self.profile.tablet.get()
    }

    fn screen_window_ratio(&mut self) -> f32 {
        self.profile.window_ratio.get()
    }

    fn safe_inset_left(&mut self) -> i32 {
        self.profile.side_inset.get()
    }

    fn safe_inset_top(&mut self) -> i32 {
        self.profile.top_inset.get()
    }

    fn safe_inset_right(&mut self) -> i32 {
        self.profile.side_inset.get()
    }

    fn safe_inset_bottom(&mut self) -> i32 {
        self.profile.bottom_inset.get()
    }

    fn system_clock_now(&mut self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_micros() as i64)
    }

    fn vibrate(&mut self, _gate: f64, _duration: f64, _strength: f64) {}

    fn cancel_vibration(&mut self) {}

    fn has_vibrator(&mut self) -> bool {
        true
    }

    fn web_view_is_open(&mut self) -> bool {
        false
    }

    fn share_image(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) {}

    fn has_inquiry_code(&mut self) -> bool {
        false
    }

    fn connecting_show(&mut self, _text: &[u8]) {}

    fn connecting_hide(&mut self) {}

    fn ranking_submit(&mut self, _entry: i32, _score: i32) {}

    fn is_network_available(&mut self) -> bool {
        false
    }

    fn feature_enabled(&mut self, _feature: i32) -> bool {
        false
    }

    fn config_int(&mut self, _key: &[u8], min: i32, _max: i32) -> i32 {
        min
    }

    fn reward_ad_ready(&mut self, _kind: i32, _flag: i32) -> bool {
        false
    }

    fn show_rewarded_ad(&mut self, _kind: i32) {}

    fn web_popup_open(&mut self, _kind: i32, _map: i32, _stage: i32) {}

    fn labyrinth_submit(&mut self, _cleared: i32, _units: i32) {}

    fn ad_prepare(&mut self, _kind: i32) {}

    fn ad_showing(&mut self) -> bool {
        false
    }

    fn event_schedule_active(&mut self, _now: f64) -> Vec<Vec<i32>> {
        Vec::new()
    }

    fn config_json_int(&mut self, _section: &[u8], _name: &[u8]) -> Option<i32> {
        None
    }

    fn notification_schedule(&mut self, _flag: u8, _kind: i32) {}

    fn tapjoy_showing(&mut self) -> bool {
        false
    }

    fn expedition_notification_schedule(&mut self, _kind: i32, _flag: u8, _cancel: u8) {}

    fn onegai_notification_schedule(&mut self, _flag: u8, _cancel: u8) {}

    fn event_calendar_notification_schedule(&mut self, _flag: u8, _cancel: u8) {}

    fn comeback_notification_schedule(&mut self, _flag: u8, _cancel: u8) {}

    fn time_manager_request(&mut self) {}

    fn inquiry_code(&mut self) -> Vec<u8> {
        Vec::new()
    }

    fn adjust_track_event(&mut self, _token: &[u8], _value: &[u8]) {}

    fn localize(&mut self, text: &[u8]) -> Vec<u8> {
        text.to_vec()
    }
}
