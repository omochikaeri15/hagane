const SLOTS: usize = 8192;
const MASK: usize = SLOTS - 1;
const LIMIT: u32 = 4096;
const WIDEST: u32 = 12;
const BLOCK: usize = 255;
const SPREAD: u32 = 0x9E37_79B1;

struct Bits {
    out: Vec<u8>,
    held: u64,
    count: u32,
}

impl Bits {
    fn put(&mut self, code: u32, width: u32) {
        self.held |= u64::from(code) << self.count;
        self.count += width;

        while self.count >= 8 {
            self.out.push(self.held as u8);
            self.held >>= 8;
            self.count -= 8;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.out.push(self.held as u8);
        }

        self.out
    }
}

pub(super) fn compress(indices: &[u8], depth: u32) -> Vec<u8> {
    let clear = 1u32 << depth;
    let end = clear + 1;
    let first = clear + 2;
    let mut keys = vec![0u32; SLOTS];
    let mut codes = vec![0u16; SLOTS];
    let mut bits = Bits { out: Vec::with_capacity(indices.len() / 3 + 16), held: 0, count: 0 };
    let mut width = depth + 1;
    let mut next = first;

    bits.put(clear, width);

    let Some((head, rest)) = indices.split_first() else {
        bits.put(end, width);
        return bits.finish();
    };
    let mut prefix = u32::from(*head);

    for symbol in rest {
        let key = ((prefix << 8) | u32::from(*symbol)) + 1;
        let mut slot = (key.wrapping_mul(SPREAD) >> 19) as usize & MASK;
        let found = loop {
            match keys[slot] {
                0 => break None,
                held if held == key => break Some(u32::from(codes[slot])),
                _ => slot = (slot + 1) & MASK,
            }
        };

        if let Some(code) = found {
            prefix = code;
            continue;
        }

        bits.put(prefix, width);

        if next < LIMIT {
            keys[slot] = key;
            codes[slot] = next as u16;
            next += 1;

            if next > (1 << width) && width < WIDEST {
                width += 1;
            }
        } else {
            bits.put(clear, width);
            keys.fill(0);
            width = depth + 1;
            next = first;
        }

        prefix = u32::from(*symbol);
    }

    bits.put(prefix, width);

    if next < LIMIT && next + 1 > (1 << width) && width < WIDEST {
        width += 1;
    }

    bits.put(end, width);
    bits.finish()
}

pub(super) fn blocks(data: &[u8], out: &mut Vec<u8>) {
    for part in data.chunks(BLOCK) {
        out.push(part.len() as u8);
        out.extend_from_slice(part);
    }

    out.push(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(data: &[u8], depth: u32) -> Vec<u8> {
        let clear = 1u32 << depth;
        let end = clear + 1;
        let mut table: Vec<Vec<u8>> = Vec::new();
        let reset = |table: &mut Vec<Vec<u8>>| {
            table.clear();
            table.extend((0..clear).map(|value| vec![value as u8]));
            table.push(Vec::new());
            table.push(Vec::new());
        };
        let mut out = Vec::new();
        let mut width = depth + 1;
        let mut held = 0u64;
        let mut count = 0;
        let mut bytes = data.iter();
        let mut previous: Option<Vec<u8>> = None;

        reset(&mut table);

        loop {
            while count < width {
                held |= u64::from(*bytes.next().expect("the stream ends with its end code")) << count;
                count += 8;
            }

            let code = (held & ((1 << width) - 1)) as u32;
            held >>= width;
            count -= width;

            if code == clear {
                reset(&mut table);
                width = depth + 1;
                previous = None;
                continue;
            }

            if code == end {
                return out;
            }

            let entry = match (table.get(code as usize), &previous) {
                (Some(entry), _) => entry.clone(),
                (None, Some(previous)) => previous.iter().copied().chain([previous[0]]).collect(),
                (None, None) => panic!("a code came before its entry"),
            };

            out.extend_from_slice(&entry);

            if let Some(previous) = previous
                && table.len() < LIMIT as usize
            {
                table.push(previous.iter().copied().chain([entry[0]]).collect());
            }

            if table.len() == 1 << width && width < WIDEST {
                width += 1;
            }

            previous = Some(entry);
        }
    }

    #[test]
    fn streams_expand_back_across_width_changes_and_resets() {
        let noisy: Vec<u8> = (0..200_000u32).map(|n| (n.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let flat = vec![7u8; 70_000];
        let ramp: Vec<u8> = (0..9_000u32).map(|n| (n % 256) as u8).collect();

        for (input, depth) in [(&noisy, 8), (&flat, 8), (&ramp, 8)] {
            assert_eq!(&expand(&compress(input, depth), depth), input);
        }

        // every length around the points where the code width grows, on a tiny alphabet
        for length in 0..600 {
            let input: Vec<u8> = (0..length as u32).map(|n| ((n * n + n / 3) % 4) as u8).collect();
            assert_eq!(expand(&compress(&input, 2), 2), input, "length {length}");
        }
    }
}
