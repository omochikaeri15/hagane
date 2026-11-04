use emulator::engine::Platform;
use emulator::runtime::InertPlatform;

use crate::platform;

const MILLIS_PER_SECOND: f64 = 1000.0;

pub(super) struct DevicePlatform {
    inert: InertPlatform,
}

impl DevicePlatform {
    pub(super) fn new(inert: InertPlatform) -> Self {
        Self { inert }
    }
}

impl Platform for DevicePlatform {
    fn set_keep_awake(&mut self, awake: bool) {
        self.inert.set_keep_awake(awake);
    }

    fn is_tablet(&mut self) -> bool {
        self.inert.is_tablet()
    }

    fn ad_showing(&mut self) -> bool {
        self.inert.ad_showing()
    }

    fn screen_window_ratio(&mut self) -> f32 {
        self.inert.screen_window_ratio()
    }

    fn safe_inset_left(&mut self) -> i32 {
        self.inert.safe_inset_left()
    }

    fn safe_inset_top(&mut self) -> i32 {
        self.inert.safe_inset_top()
    }

    fn safe_inset_right(&mut self) -> i32 {
        self.inert.safe_inset_right()
    }

    fn safe_inset_bottom(&mut self) -> i32 {
        self.inert.safe_inset_bottom()
    }

    fn system_clock_now(&mut self) -> i64 {
        self.inert.system_clock_now()
    }

    fn vibrate(&mut self, delay: f64, duration: f64, _amplitude: f64) {
        platform::vibrate(delay as f32, (duration * MILLIS_PER_SECOND) as i32);
    }

    fn cancel_vibration(&mut self) {
        platform::cancel_vibration();
    }

    fn has_vibrator(&mut self) -> bool {
        platform::has_vibrator()
    }

    fn web_view_is_open(&mut self) -> bool {
        self.inert.web_view_is_open()
    }

    fn share_image(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.inert.share_image(x, y, width, height);
    }

    fn has_inquiry_code(&mut self) -> bool {
        self.inert.has_inquiry_code()
    }

    fn connecting_show(&mut self, text: &[u8]) {
        self.inert.connecting_show(text);
    }

    fn connecting_hide(&mut self) {
        self.inert.connecting_hide();
    }

    fn ranking_submit(&mut self, entry: i32, score: i32) {
        self.inert.ranking_submit(entry, score);
    }

    fn is_network_available(&mut self) -> bool {
        self.inert.is_network_available()
    }

    fn feature_enabled(&mut self, feature: i32) -> bool {
        self.inert.feature_enabled(feature)
    }

    fn config_int(&mut self, key: &[u8], min: i32, max: i32) -> i32 {
        self.inert.config_int(key, min, max)
    }

    fn reward_ad_ready(&mut self, kind: i32, flag: i32) -> bool {
        self.inert.reward_ad_ready(kind, flag)
    }

    fn show_rewarded_ad(&mut self, kind: i32) {
        self.inert.show_rewarded_ad(kind);
    }

    fn web_popup_open(&mut self, kind: i32, map: i32, stage: i32) {
        self.inert.web_popup_open(kind, map, stage);
    }

    fn labyrinth_submit(&mut self, cleared: i32, units: i32) {
        self.inert.labyrinth_submit(cleared, units);
    }

    fn ad_prepare(&mut self, kind: i32) {
        self.inert.ad_prepare(kind);
    }

    fn event_schedule_active(&mut self, now: f64) -> Vec<Vec<i32>> {
        self.inert.event_schedule_active(now)
    }

    fn config_json_int(&mut self, section: &[u8], name: &[u8]) -> Option<i32> {
        self.inert.config_json_int(section, name)
    }

    fn notification_schedule(&mut self, flag: u8, kind: i32) {
        self.inert.notification_schedule(flag, kind);
    }

    fn tapjoy_showing(&mut self) -> bool {
        self.inert.tapjoy_showing()
    }

    fn expedition_notification_schedule(&mut self, kind: i32, flag: u8, cancel: u8) {
        self.inert.expedition_notification_schedule(kind, flag, cancel);
    }

    fn onegai_notification_schedule(&mut self, flag: u8, cancel: u8) {
        self.inert.onegai_notification_schedule(flag, cancel);
    }

    fn event_calendar_notification_schedule(&mut self, flag: u8, cancel: u8) {
        self.inert.event_calendar_notification_schedule(flag, cancel);
    }

    fn comeback_notification_schedule(&mut self, flag: u8, cancel: u8) {
        self.inert.comeback_notification_schedule(flag, cancel);
    }

    fn time_manager_request(&mut self) {
        self.inert.time_manager_request();
    }

    fn inquiry_code(&mut self) -> Vec<u8> {
        self.inert.inquiry_code()
    }

    fn adjust_track_event(&mut self, token: &[u8], value: &[u8]) {
        self.inert.adjust_track_event(token, value);
    }

    fn localize(&mut self, text: &[u8]) -> Vec<u8> {
        self.inert.localize(text)
    }
}
