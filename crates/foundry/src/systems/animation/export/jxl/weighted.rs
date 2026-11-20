const DIVIDE: [u32; 64] = [
    16_777_216, 8_388_608, 5_592_405, 4_194_304, 3_355_443, 2_796_202, 2_396_745, 2_097_152, 1_864_135, 1_677_721, 1_525_201, 1_398_101, 1_290_555, 1_198_372, 1_118_481, 1_048_576, 986_895, 932_067, 883_011, 838_860, 798_915, 762_600, 729_444, 699_050,
    671_088, 645_277, 621_378, 599_186, 578_524, 559_240, 541_200, 524_288, 508_400, 493_447, 479_349, 466_033, 453_438, 441_505, 430_185, 419_430, 409_200, 399_457, 390_167, 381_300, 372_827, 364_722, 356_962, 349_525, 342_392, 335_544,
    328_965, 322_638, 316_551, 310_689, 305_040, 299_593, 294_337, 289_262, 284_359, 279_620, 275_036, 270_600, 266_305, 262_144,
];
const WEIGHTS: [u32; 4] = [13, 12, 12, 12];
const EXTRA: u32 = 3;
const ROUND: i64 = 3;

pub(super) struct Weighted {
    width: usize,
    error: Vec<i64>,
    misses: [Vec<u32>; 4],
    guesses: [i64; 4],
    guess: i64,
}

pub(super) struct Near {
    pub(super) north: i64,
    pub(super) west: i64,
    pub(super) east: i64,
}

#[inline]
fn weight(error: u64, most: u32) -> u32 {
    let shift = ((error + 1).ilog2() as i32 - 5).max(0) as u32;
    4 + ((most * DIVIDE[(error >> shift) as usize]) >> shift)
}

#[inline]
fn blend(guesses: &[i64; 4], mut weights: [u32; 4]) -> i64 {
    let total: u32 = weights.iter().sum();
    let shift = total.ilog2() - 4;
    for weight in &mut weights {
        *weight >>= shift;
    }
    let total: u32 = weights.iter().sum();
    let mut sum = i64::from(total >> 1) - 1;
    for (guess, weight) in guesses.iter().zip(weights) {
        sum += guess * i64::from(weight);
    }
    (sum * i64::from(DIVIDE[total as usize - 1])) >> 24
}

impl Weighted {
    pub(super) fn new() -> Self {
        Self { width: 0, error: Vec::new(), misses: [Vec::new(), Vec::new(), Vec::new(), Vec::new()], guesses: [0; 4], guess: 0 }
    }

    pub(super) fn reset(&mut self, width: usize) {
        self.width = width;
        let size = (width + 2) * 2;
        self.error.clear();
        self.error.resize(size, 0);
        for misses in &mut self.misses {
            misses.clear();
            misses.resize(size, 0);
        }
    }

    #[inline]
    pub(super) fn predict(&mut self, x: usize, y: usize, near: &Near) -> (i64, i64) {
        let (current, previous) = if y & 1 == 1 { (0, self.width + 2) } else { (self.width + 2, 0) };
        let north_at = previous + x;
        let east_at = if x + 1 < self.width { north_at + 1 } else { north_at };
        let corner_at = if x > 0 { north_at - 1 } else { north_at };
        let weights: [u32; 4] = std::array::from_fn(|index| {
            let misses = &self.misses[index];
            weight(u64::from(misses[north_at]) + u64::from(misses[east_at]) + u64::from(misses[corner_at]), WEIGHTS[index])
        });
        let (north, west, east) = (near.north << EXTRA, near.west << EXTRA, near.east << EXTRA);
        let error_west = if x == 0 { 0 } else { self.error[current + x - 1] };
        let (error_north, error_corner, error_east) = (self.error[north_at], self.error[corner_at], self.error[east_at]);
        let both = error_north + error_west;
        let mut worst = error_west;
        for candidate in [error_north, error_corner, error_east] {
            if candidate.abs() > worst.abs() {
                worst = candidate;
            }
        }
        self.guesses = [
            west + east - north,
            north - (((both + error_east) * 16) >> 5),
            west - (((both + error_corner) * 10) >> 5),
            north - ((error_corner * 7 + error_north * 7 + error_east * 7) >> 5),
        ];
        self.guess = blend(&self.guesses, weights);
        if ((error_north ^ error_west) | (error_north ^ error_corner)) <= 0 {
            let high = west.max(east).max(north);
            let low = west.min(east).min(north);
            self.guess = self.guess.clamp(low, high);
        }
        ((self.guess + ROUND) >> EXTRA, worst)
    }

    #[inline]
    pub(super) fn update(&mut self, value: i64, x: usize, y: usize) {
        let (current, previous) = if y & 1 == 1 { (0, self.width + 2) } else { (self.width + 2, 0) };
        let value = value << EXTRA;
        self.error[current + x] = self.guess - value;
        for (misses, guess) in self.misses.iter_mut().zip(self.guesses) {
            let miss = (((guess - value).abs() + ROUND) >> EXTRA) as u32;
            misses[current + x] = miss;
            misses[previous + x + 1] += miss;
        }
    }
}
