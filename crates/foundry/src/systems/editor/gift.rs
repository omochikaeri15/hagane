pub(super) const TABLE: &str = "rankGift.csv";

pub(super) const TITLE: &str = "Rank Reward";

pub(super) const FOLDER: &str = "Rank Rewards";

pub(super) const NOUN: &str = "Rank";

pub fn rewards(line: &str) -> Vec<(i64, i64)> {
    let cells: Vec<i64> = line
        .split(DELIMITER)
        .skip(1)
        .map(|cell| cell.trim().parse::<i64>().unwrap_or(0))
        .take_while(|cell| *cell != TERMINATOR)
        .collect();

    cells.as_chunks::<2>().0.iter().map(|[item, quantity]| (*item, *quantity)).collect()
}

pub const DELIMITER: char = ',';

pub const TERMINATOR: i64 = -1;
