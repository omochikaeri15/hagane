pub(super) const BLOCK: usize = 64;

pub(super) const FREQUENCY: [u8; BLOCK] = [
    0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 23, 23, 24, 24, 24, 24, 25, 25, 25, 25, 26, 26, 26, 26, 27, 27, 27, 27, 28, 28, 28, 28, 29, 29, 29, 29, 30,
    30, 30, 30,
];

pub(super) const REMAINING: [u16; BLOCK] = [
    0, 0, 31, 62, 62, 93, 93, 93, 93, 123, 123, 123, 123, 152, 152, 152, 152, 152, 152, 152, 152, 180, 180, 180, 180, 180, 180, 180, 180, 180, 180, 180, 180, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206,
    206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206, 206,
];

pub(super) const REMAINING_STARTS: [u16; 8] = [0, 31, 62, 93, 123, 152, 180, 206];
pub(super) const COUNT_BUCKETS: usize = 37;
pub(super) const DENSITY: usize = 458;

pub(super) const ORDER: [u8; BLOCK] = zigzag();

const fn zigzag() -> [u8; BLOCK] {
    let mut order = [0u8; BLOCK];
    let mut filled = 0;
    let mut diagonal = 0;
    while diagonal < 15 {
        let mut step = 0;
        while step <= diagonal {
            let (x, y) = if diagonal % 2 == 1 { (diagonal - step, step) } else { (step, diagonal - step) };
            if x < 8 && y < 8 {
                order[filled] = (y * 8 + x) as u8;
                filled += 1;
            }
            step += 1;
        }
        diagonal += 1;
    }
    order
}

#[cfg(test)]
mod tests {
    use super::ORDER;

    #[test]
    fn order_starts_like_the_reference_zigzag() {
        assert_eq!(&ORDER[..16], &[0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5]);
        let mut seen = [false; 64];
        for at in ORDER {
            seen[usize::from(at)] = true;
        }
        assert!(seen.iter().all(|hit| *hit));
        assert_eq!(ORDER[63], 63);
    }
}
