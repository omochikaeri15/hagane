pub fn roguelike_icon_ability(icon: i32) -> i32 {
    match icon {
        0x2d => 0x3f2,
        0x2e => 0x3f3,
        _ => -1,
    }
}
