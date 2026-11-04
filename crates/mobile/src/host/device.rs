use emulator::engine::{AssetSource, Platform};
use emulator::runtime::InertPlatform;

use crate::platform;

use super::assets::DiskAssets;

const MILLIS_PER_SECOND: f64 = 1000.0;
const VIBRATION_FILE: &[u8] = b"Vibration.csv";
const IOS_KIND: f64 = 0.0;
const SHORTEST_SECONDS: f64 = 0.02;
const DEFAULT_SHARPNESS: f64 = 0.5;
const SAME: f64 = 1e-6;

pub(super) struct DevicePlatform {
    inert: InertPlatform,
    assets: DiskAssets,
    rows: Option<Vec<[f64; 5]>>,
}

impl DevicePlatform {
    pub(super) fn new(inert: InertPlatform, assets: DiskAssets) -> Self {
        Self { inert, assets, rows: None }
    }

    fn ios_row(&mut self, strength: f64, seconds: f64) -> (f64, f64, f64) {
        let rows = self.rows.get_or_insert_with(|| self.assets.open(VIBRATION_FILE, 0, 0).map_or_else(Vec::new, |bytes| rows(&String::from_utf8_lossy(&bytes))));

        ios_twin(rows, strength, seconds).unwrap_or((strength, seconds, DEFAULT_SHARPNESS))
    }
}

fn rows(csv: &str) -> Vec<[f64; 5]> {
    csv.lines().map_while(|line| line.split(',').map(|cell| cell.trim().parse().ok()).collect::<Option<Vec<f64>>>()?.try_into().ok()).collect()
}

fn ios_twin(rows: &[[f64; 5]], strength: f64, seconds: f64) -> Option<(f64, f64, f64)> {
    let same = |left: f64, right: f64| (left - right).abs() < SAME;
    let [id, ..] = rows.iter().find(|[_, kind, power, millis, _]| *kind != IOS_KIND && same(*power, strength) && same((millis / MILLIS_PER_SECOND).max(SHORTEST_SECONDS), seconds))?;

    rows.iter().find(|[row, kind, ..]| row == id && *kind == IOS_KIND).map(|[_, _, power, millis, sharpness]| (*power, millis / MILLIS_PER_SECOND, *sharpness))
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
        let (strength, seconds, sharpness) = if cfg!(target_os = "ios") { self.ios_row(delay, duration) } else { (delay, duration, DEFAULT_SHARPNESS) };

        platform::vibrate(strength as f32, (seconds * MILLIS_PER_SECOND) as i32, sharpness as f32);
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

#[cfg(test)]
mod tests {
    use super::*;

    // The engine only ever hands over the Android row, so iOS finds its own through the shared id.
    #[test]
    fn an_android_row_finds_the_ios_row_with_its_id() {
        let table = rows("0,0,0,0,1\n0,1,0,0,-1\n7,0,0.5,3,0.9\n7,1,0.25,40,-1\n8,0,0.1,50,0.2\n8,1,0.6,10,-1\n");

        assert_eq!(ios_twin(&table, 0.25, 0.04), Some((0.5, 0.003, 0.9)));
        assert_eq!(ios_twin(&table, 0.6, SHORTEST_SECONDS), Some((0.1, 0.05, 0.2)));
        assert_eq!(ios_twin(&table, 0.9, 1.0), None);
    }
}
