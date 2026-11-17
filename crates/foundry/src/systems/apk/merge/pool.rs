use std::collections::HashMap;

use super::{fit, half, span, word};

pub(super) const CHUNK: u16 = 0x0001;
const HEADER_SIZE: u16 = 28;
const HEADER: usize = 28;
const SORTED: u32 = 1;
const UTF8: u32 = 1 << 8;
const END: u32 = 0xFFFF_FFFF;
const SPAN: usize = 12;
const LONG_NARROW: usize = 0x7F;
const MAX_NARROW: usize = 0x7FFF;
const LONG_WIDE: usize = 0x7FFF;
const MAX_WIDE: usize = 0x7FFF_FFFF;
const HIGH_SURROGATE: u32 = 0xD800;
const LOW_SURROGATE: u32 = 0xDC00;
const SUPPLEMENTARY: u32 = 0x1_0000;
const REPLACEMENT: u32 = 0xFFFD;

#[derive(Clone, Copy)]
struct Span {
    name: u32,
    first: u32,
    last: u32,
}

pub(super) struct Pool {
    utf8: bool,
    flags: u32,
    items: Vec<Vec<u8>>,
    styles: Vec<Vec<Span>>,
}

fn narrow_prefix(chunk: &[u8], at: usize) -> Result<(usize, usize), String> {
    let lead = usize::from(*chunk.get(at).ok_or("A string pool ends inside a string")?);

    if lead & 0x80 == 0 {
        return Ok((lead, 1));
    }

    let low = usize::from(*chunk.get(at + 1).ok_or("A string pool ends inside a string")?);

    Ok(((lead & 0x7F) << 8 | low, 2))
}

fn wide_prefix(chunk: &[u8], at: usize) -> Result<(usize, usize), String> {
    let lead = usize::from(half(chunk, at)?);

    if lead & 0x8000 == 0 {
        return Ok((lead, 2));
    }

    Ok(((lead & 0x7FFF) << 16 | usize::from(half(chunk, at + 2)?), 4))
}

fn narrow_item(chunk: &[u8], at: usize) -> Result<usize, String> {
    let (_, counted) = narrow_prefix(chunk, at)?;
    let (bytes, sized) = narrow_prefix(chunk, at + counted)?;

    Ok(counted + sized + bytes + 1)
}

fn wide_item(chunk: &[u8], at: usize) -> Result<usize, String> {
    let (units, sized) = wide_prefix(chunk, at)?;

    Ok(sized + units * 2 + 2)
}

fn widen(bytes: &[u8]) -> Vec<u16> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut at = 0;

    while let Some(&lead) = bytes.get(at) {
        let (length, start) = match lead {
            0x00..=0x7F => (1, u32::from(lead)),
            0xC0..=0xDF => (2, u32::from(lead & 0x1F)),
            0xE0..=0xEF => (3, u32::from(lead & 0x0F)),
            0xF0..=0xF7 => (4, u32::from(lead & 0x07)),
            _ => (1, REPLACEMENT),
        };
        let point = (1..length).fold(start, |held, more| held << 6 | u32::from(bytes.get(at + more).map_or(0, |next| next & 0x3F)));

        at += length;

        if point >= SUPPLEMENTARY {
            let rest = point - SUPPLEMENTARY;

            units.push(u16::try_from(HIGH_SURROGATE + (rest >> 10)).unwrap_or(u16::MAX));
            units.push(u16::try_from(LOW_SURROGATE + (rest & 0x3FF)).unwrap_or(u16::MAX));
        } else {
            units.push(u16::try_from(point).unwrap_or(u16::MAX));
        }
    }

    units
}

fn shrink(units: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(units.len());

    for decoded in char::decode_utf16(units.iter().copied()) {
        match decoded {
            Ok(letter) => bytes.extend(letter.encode_utf8(&mut [0; 4]).as_bytes()),
            Err(lone) => {
                let unit = lone.unpaired_surrogate();

                bytes.extend([0xE0 | (unit >> 12).to_le_bytes()[0], 0x80 | ((unit >> 6) & 0x3F).to_le_bytes()[0], 0x80 | (unit & 0x3F).to_le_bytes()[0]]);
            }
        }
    }

    bytes
}

fn push_narrow(item: &mut Vec<u8>, length: usize) -> Result<(), String> {
    if length > MAX_NARROW {
        return Err("A string is too long for a UTF-8 string pool".to_string());
    }

    let bytes = fit(length)?.to_le_bytes();

    if length > LONG_NARROW {
        item.push(bytes[1] | 0x80);
    }

    item.push(bytes[0]);

    Ok(())
}

impl Pool {
    pub(super) fn parse(chunk: &[u8]) -> Result<Self, String> {
        if half(chunk, 0)? != CHUNK {
            return Err("A string pool was expected and something else was found".to_string());
        }

        let header = usize::from(half(chunk, 2)?);
        let (strings, styled) = (span(chunk, 8)?, span(chunk, 12)?);
        let flags = word(chunk, 16)?;
        let (strings_start, styles_start) = (span(chunk, 20)?, span(chunk, 24)?);
        let utf8 = flags & UTF8 != 0;
        let mut items = Vec::with_capacity(strings);

        for index in 0..strings {
            let at = strings_start + span(chunk, header + index * 4)?;
            let length = if utf8 { narrow_item(chunk, at)? } else { wide_item(chunk, at)? };

            items.push(chunk.get(at..at + length).ok_or("A string pool ends inside a string")?.to_vec());
        }

        let mut styles = Vec::with_capacity(styled);

        for index in 0..styled {
            let mut at = styles_start + span(chunk, header + (strings + index) * 4)?;
            let mut spans = Vec::new();

            loop {
                let name = word(chunk, at)?;

                if name == END {
                    break;
                }

                spans.push(Span { name, first: word(chunk, at + 4)?, last: word(chunk, at + 8)? });
                at += SPAN;
            }

            styles.push(spans);
        }

        Ok(Self { utf8, flags, items, styles })
    }

    #[cfg(test)]
    pub(super) fn of(utf8: bool, texts: &[&str]) -> Self {
        let mut pool = Self { utf8, flags: if utf8 { UTF8 } else { 0 }, items: Vec::new(), styles: Vec::new() };

        for text in texts {
            pool.push(text).unwrap();
        }

        pool
    }

    pub(super) fn len(&self) -> usize {
        self.items.len()
    }

    fn units(&self, item: &[u8]) -> Result<Vec<u16>, String> {
        if self.utf8 {
            let (_, counted) = narrow_prefix(item, 0)?;
            let (bytes, sized) = narrow_prefix(item, counted)?;
            let from = counted + sized;

            return Ok(widen(item.get(from..from + bytes).ok_or("A string pool ends inside a string")?));
        }

        let (units, sized) = wide_prefix(item, 0)?;

        (0..units).map(|index| half(item, sized + index * 2)).collect()
    }

    fn encode(&self, units: &[u16]) -> Result<Vec<u8>, String> {
        let mut item = Vec::with_capacity(units.len() * 2 + 4);

        if self.utf8 {
            let bytes = shrink(units);

            push_narrow(&mut item, units.len())?;
            push_narrow(&mut item, bytes.len())?;
            item.extend(bytes);
            item.push(0);

            return Ok(item);
        }

        if units.len() > MAX_WIDE {
            return Err("A string is too long for a string pool".to_string());
        }

        let length = fit(units.len())?.to_le_bytes();

        if units.len() > LONG_WIDE {
            item.extend([length[2], length[3] | 0x80]);
        }

        item.extend([length[0], length[1]]);
        item.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
        item.extend([0, 0]);

        Ok(item)
    }

    pub(super) fn text(&self, index: usize) -> Result<String, String> {
        let item = self.items.get(index).ok_or("A string index points outside its pool")?;

        Ok(String::from_utf16_lossy(&self.units(item)?))
    }

    pub(super) fn push(&mut self, text: &str) -> Result<(), String> {
        let item = self.encode(&text.encode_utf16().collect::<Vec<_>>())?;

        self.items.push(item);
        self.flags &= !SORTED;

        Ok(())
    }

    fn plain(&self, index: usize) -> bool {
        self.styles.get(index).is_none_or(Vec::is_empty)
    }

    pub(super) fn absorb(&mut self, other: Self) -> Result<Vec<u32>, String> {
        let same = other.utf8 == self.utf8;
        let known: HashMap<&[u8], usize> = self.items.iter().enumerate().filter(|(index, _)| self.plain(*index)).map(|(index, item)| (item.as_slice(), index)).collect();
        let mut staged = Vec::with_capacity(other.items.len());

        for (index, item) in other.items.iter().enumerate() {
            let item = if same { None } else { Some(self.encode(&other.units(item)?)?) };
            let sought = item.as_deref().or_else(|| other.items.get(index).map(Vec::as_slice)).unwrap_or_default();
            let found = if other.plain(index) { known.get(sought).copied() } else { None };

            staged.push((item, found));
        }

        drop(known);

        let mut map = Vec::with_capacity(staged.len());

        for ((converted, found), original) in staged.into_iter().zip(other.items) {
            let landed = found.unwrap_or(self.items.len());

            if found.is_none() {
                self.items.push(converted.unwrap_or(original));
            }

            map.push(fit(landed)?);
        }

        for (index, spans) in other.styles.into_iter().enumerate() {
            if spans.is_empty() {
                continue;
            }

            let target = map.get(index).copied().and_then(|held| usize::try_from(held).ok()).ok_or("A style points outside its pool")?;
            let moved = spans
                .into_iter()
                .map(|held| {
                    let name = usize::try_from(held.name).ok().and_then(|name| map.get(name)).copied().ok_or("A style tag points outside its pool")?;

                    Ok(Span { name, ..held })
                })
                .collect::<Result<Vec<_>, String>>()?;

            if self.styles.len() <= target {
                self.styles.resize_with(target + 1, Vec::new);
            }

            if let Some(slot) = self.styles.get_mut(target) {
                *slot = moved;
            }
        }

        self.flags &= !SORTED;

        Ok(map)
    }

    pub(super) fn write(&self) -> Result<Vec<u8>, String> {
        let (strings, styled) = (self.items.len(), self.styles.len());
        let mut offsets = Vec::with_capacity((strings + styled) * 4);
        let mut data = Vec::new();

        for item in &self.items {
            offsets.extend(fit(data.len())?.to_le_bytes());
            data.extend(item);
        }

        data.resize(data.len().next_multiple_of(4), 0);

        let mut spans = Vec::new();

        for style in &self.styles {
            offsets.extend(fit(spans.len())?.to_le_bytes());

            for held in style {
                spans.extend([held.name, held.first, held.last].into_iter().flat_map(u32::to_le_bytes));
            }

            spans.extend(END.to_le_bytes());
        }

        if styled > 0 {
            spans.extend([END, END].into_iter().flat_map(u32::to_le_bytes));
        }

        let strings_start = HEADER + offsets.len();
        let styles_start = if styled > 0 { strings_start + data.len() } else { 0 };
        let size = strings_start + data.len() + spans.len();
        let mut chunk = Vec::with_capacity(size);

        chunk.extend(CHUNK.to_le_bytes());
        chunk.extend(HEADER_SIZE.to_le_bytes());

        for field in [size, strings, styled] {
            chunk.extend(fit(field)?.to_le_bytes());
        }

        chunk.extend(self.flags.to_le_bytes());
        chunk.extend(fit(strings_start)?.to_le_bytes());
        chunk.extend(fit(styles_start)?.to_le_bytes());
        chunk.extend(offsets);
        chunk.extend(data);
        chunk.extend(spans);

        Ok(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pool_reads_back_what_it_wrote() {
        for utf8 in [true, false] {
            let long = "x".repeat(300);
            let written = Pool::of(utf8, &["res/a.png", "日本語 \u{1F408}", &long, ""]).write().unwrap();
            let read = Pool::parse(&written).unwrap();

            assert_eq!(read.len(), 4);
            assert_eq!(read.text(0).unwrap(), "res/a.png");
            assert_eq!(read.text(1).unwrap(), "日本語 \u{1F408}");
            assert_eq!(read.text(2).unwrap(), long);
            assert_eq!(read.write().unwrap(), written);
        }
    }

    #[test]
    fn absorbing_reuses_plain_strings_and_crosses_encodings() {
        let mut base = Pool::of(true, &["shared", "base only"]);
        let map = base.absorb(Pool::of(false, &["split only", "shared", "ねこ"])).unwrap();

        assert_eq!(map, [2, 0, 3]);
        assert_eq!(base.text(2).unwrap(), "split only");
        assert_eq!(base.text(3).unwrap(), "ねこ");
    }

    // A styled string keeps its spans, and the tag names they point at move with it.
    #[test]
    fn a_styled_string_lands_past_the_plain_ones_with_its_tags_remapped() {
        let mut base = Pool::of(true, &["plain", "b"]);
        let mut split = Pool::of(true, &["bold text", "b"]);

        split.styles.push(vec![Span { name: 1, first: 0, last: 3 }]);

        let map = base.absorb(split).unwrap();
        let read = Pool::parse(&base.write().unwrap()).unwrap();

        assert_eq!(map, [2, 1]);
        assert_eq!(read.styles.len(), 3);
        assert!(read.styles[0].is_empty() && read.styles[1].is_empty());
        assert_eq!((read.styles[2][0].name, read.styles[2][0].first, read.styles[2][0].last), (1, 0, 3));
    }
}
