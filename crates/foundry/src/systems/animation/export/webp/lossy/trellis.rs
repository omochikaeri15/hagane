use super::quant::Steps;
use super::rate::Rates;
use super::tables::ZIGZAG;

const WEIGHTS: [i64; 16] = [30, 27, 19, 11, 27, 24, 17, 10, 19, 17, 12, 8, 11, 10, 8, 6];
const CEILING: i64 = 2047;

#[derive(Clone, Copy)]
struct State {
    score: i64,
    context: usize,
}

pub(super) struct Lane<'a> {
    pub(super) rates: &'a Rates,
    pub(super) kind: usize,
    pub(super) context: usize,
    pub(super) lambda: u64,
}

pub(super) fn quantize(coefficients: &[i16; 16], steps: &Steps, first: usize, lane: &Lane<'_>) -> ([i16; 16], bool) {
    let lambda = lane.lambda as i64;
    let (rates, kind) = (lane.rates, lane.kind);
    let threshold = i64::from(steps.size(1)) * i64::from(steps.size(1)) / 4;
    let mut last = (first..16).rev().find(|&place| i64::from(coefficients[ZIGZAG[place]]).pow(2) > threshold).unwrap_or(first);
    if last < 15 {
        last += 1;
    }
    let mut best_score = lambda * i64::from(rates.stop(kind, first, lane.context));
    let mut best: Option<(usize, usize)> = None;
    let mut nodes = [[(0i64, 0usize); 2]; 16];
    let start = State { score: lambda * i64::from(rates.more(kind, first, lane.context)), context: lane.context };
    let mut previous = [Some(start), Some(start)];
    for place in first..=last {
        let index = ZIGZAG[place];
        let step = i64::from(steps.size(index));
        let size = i64::from(coefficients[index].unsigned_abs());
        let floor = (size / step).min(CEILING);
        let nearest = ((size + step / 2) / step).min(CEILING);
        let mut current = [None, None];
        for (offset, slot) in current.iter_mut().enumerate() {
            let level = floor + offset as i64;
            if level > nearest {
                continue;
            }
            let error = size - level * step;
            let base = 256 * WEIGHTS[index] * (error * error - size * size);
            let mut chosen: Option<(i64, usize)> = None;
            for (from, state) in previous.iter().enumerate() {
                let Some(state) = state else {
                    continue;
                };
                let more = if place > first && state.context > 0 { rates.more(kind, place, state.context) } else { 0 };
                let score = state.score + lambda * i64::from(more + rates.token(kind, place, state.context, level as u32));
                if chosen.is_none_or(|(kept, _)| score < kept) {
                    chosen = Some((score, from));
                }
            }
            let Some((score, from)) = chosen else {
                continue;
            };
            let score = score + base;
            let context = level.min(2) as usize;
            nodes[place][offset] = (level, from);
            *slot = Some(State { score, context });
            if level != 0 {
                let ending = if place < 15 { rates.stop(kind, place + 1, context) } else { 0 };
                let total = score + lambda * i64::from(ending);
                if total < best_score {
                    best_score = total;
                    best = Some((place, offset));
                }
            }
        }
        previous = current;
    }
    let mut levels = [0i16; 16];
    let Some((mut place, mut offset)) = best else {
        return (levels, false);
    };
    loop {
        let (level, from) = nodes[place][offset];
        let signed = if coefficients[ZIGZAG[place]] < 0 { -level } else { level };
        levels[place] = signed as i16;
        if place == first {
            break;
        }
        place -= 1;
        offset = from;
    }
    (levels, true)
}

#[cfg(test)]
mod tests {
    use super::super::quant::Quantizer;
    use super::super::rate::Rates;
    use super::super::tables::DEFAULT_PROBS;
    use super::{Lane, quantize};

    #[test]
    fn free_rate_falls_back_to_rounding() {
        // With almost no weight on rate the trellis must land on the nearest level everywhere, which checks the
        // path search and the backtracking against the plain quantizer.
        let rates = Rates::new(&DEFAULT_PROBS);
        let steps = Quantizer::new(30, 0).luma;
        let mut state = 3u32;
        for round in 0..300 {
            let coefficients: [i16; 16] = std::array::from_fn(|place| loop {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let value = ((state >> 16) % 900) as i16 - 450;
                let step = steps.size(place);
                let tie = step % 2 == 0 && value.abs() % step == step / 2;
                if !tie {
                    break value;
                }
            });
            let first = round % 2;
            let lane = Lane { rates: &rates, kind: 3 - first * 3, context: round % 3, lambda: 0 };
            let (trellis, _) = quantize(&coefficients, &steps, first, &lane);
            let (plain, _) = steps.quantize(&coefficients, first);
            assert_eq!(trellis, plain, "round {round}");
        }
    }
}
