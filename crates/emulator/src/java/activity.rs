#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayCutout {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Activity {
    pub sdk_int: i32,
    pub display_cutout: Option<DisplayCutout>,
    pub in_multi_window: bool,
    pub display_size: (i32, i32),
    pub first_width: i32,
    pub first_height: i32,
    pub first_aspect: f32,
    pub tablet: bool,
    pub fit_width: i32,
    pub fit_height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Activity {
    pub const CUTOUT_SDK: i32 = 28;
}
