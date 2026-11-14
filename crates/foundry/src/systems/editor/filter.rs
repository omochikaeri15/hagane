pub(super) const TABLE: &str = "NyancomboFilter.tsv";

pub(super) const TITLE: &str = "Combo Filter Tab";

pub(super) const FOLDER: &str = "Combo Filter Tabs";

pub(super) const NOUN: &str = "Tab";

pub fn listed(line: &str) -> Vec<i64> {
    line.split(DELIMITER).filter_map(|cell| cell.trim().parse().ok()).collect()
}

pub const DELIMITER: char = '\t';
