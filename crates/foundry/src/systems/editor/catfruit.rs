pub(super) const TABLE: &str = "Matatabi.tsv";

pub(super) const TITLE: &str = "Catfruit";

pub(super) const FOLDER: &str = "Catfruit";

pub fn keyed(line: &str) -> Option<u32> {
    line.split(DELIMITER).next()?.trim().parse().ok()
}

pub const DELIMITER: char = '\t';
