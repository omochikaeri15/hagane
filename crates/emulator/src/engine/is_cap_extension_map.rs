use super::MapOption;

pub fn is_cap_extension_map(store: &MapOption, map_id: i32) -> bool {
    store.cap_extension_maps.contains(&map_id)
}
