use std::collections::HashSet;

use super::pool::{self, Pool};
use super::{fit, half, put_half, put_word, span, word};

const TABLE: u16 = 0x0002;
const TABLE_HEADER: u16 = 12;
const PACKAGE: u16 = 0x0200;
const TYPE: u16 = 0x0201;
const SPEC: u16 = 0x0202;
const CHUNK_HEADER: usize = 8;
const PACKAGE_ID: usize = 8;
const TYPE_STRINGS: usize = 268;
const KEY_STRINGS: usize = 276;
const KIND_ID: usize = 8;
const KIND_FLAGS: usize = 9;
const ENTRY_COUNT: usize = 12;
const ENTRIES_START: usize = 16;
const NO_ENTRY: u32 = 0xFFFF_FFFF;
const NO_SHORT_ENTRY: u16 = 0xFFFF;
const SPARSE: u8 = 1;
const SHORT: u8 = 2;
const COMPLEX: u16 = 1;
const COMPACT: u16 = 8;
const STRING: u8 = 3;
const MAP: usize = 12;
const VALUE_KIND: usize = 3;
const VALUE_DATA: usize = 4;

enum Part {
    Package(Package),
    Other(Vec<u8>),
}

pub(super) struct Table {
    values: Pool,
    parts: Vec<Part>,
}

struct Package {
    head: Vec<u8>,
    id: u32,
    types: Pool,
    keys: Pool,
    chunks: Vec<Vec<u8>>,
}

fn chunks(bytes: &[u8], from: usize) -> Result<Vec<(usize, &[u8])>, String> {
    let mut found = Vec::new();
    let mut at = from;

    while at + CHUNK_HEADER <= bytes.len() {
        let size = span(bytes, at + 4)?;

        if size < CHUNK_HEADER {
            return Err("A resource table holds an empty chunk".to_string());
        }

        found.push((at, bytes.get(at..at + size).ok_or("A resource table ends inside a chunk")?));
        at += size;
    }

    Ok(found)
}

fn kind_of(chunk: &[u8]) -> Option<(u16, u8)> {
    Some((half(chunk, 0).ok()?, *chunk.get(KIND_ID)?))
}

fn swap(chunk: &mut [u8], at: usize, map: &[u32]) -> Result<(), String> {
    let held = span(chunk, at)?;

    put_word(chunk, at, *map.get(held).ok_or("A resource points at a string outside its pool")?)
}

fn value(chunk: &mut [u8], at: usize, values: &[u32]) -> Result<(), String> {
    if chunk.get(at + VALUE_KIND) == Some(&STRING) {
        swap(chunk, at + VALUE_DATA, values)?;
    }

    Ok(())
}

fn entry(chunk: &mut [u8], at: usize, keys: Option<&[u32]>, values: &[u32]) -> Result<(), String> {
    let flags = half(chunk, at + 2)?;

    if flags & COMPACT != 0 {
        if let Some(keys) = keys {
            let moved = *keys.get(usize::from(half(chunk, at)?)).ok_or("A resource is named by a key outside its pool")?;

            put_half(chunk, at, u16::try_from(moved).map_err(|_| "A compact resource entry cannot hold its new key")?)?;
        }

        if flags.to_le_bytes()[1] == STRING {
            swap(chunk, at + VALUE_DATA, values)?;
        }

        return Ok(());
    }

    let size = usize::from(half(chunk, at)?);

    if let Some(keys) = keys {
        swap(chunk, at + 4, keys)?;
    }

    if flags & COMPLEX == 0 {
        return value(chunk, at + size, values);
    }

    for index in 0..span(chunk, at + 12)? {
        value(chunk, at + size + index * MAP + 4, values)?;
    }

    Ok(())
}

fn slots(chunk: &[u8]) -> Result<Vec<(usize, usize)>, String> {
    let header = usize::from(half(chunk, 2)?);
    let flags = *chunk.get(KIND_FLAGS).ok_or("A resource type chunk is cut short")?;
    let count = span(chunk, ENTRY_COUNT)?;
    let start = span(chunk, ENTRIES_START)?;
    let mut found = Vec::with_capacity(count);

    for index in 0..count {
        if flags & SPARSE != 0 {
            let at = header + index * 4;

            found.push((usize::from(half(chunk, at)?), start + usize::from(half(chunk, at + 2)?) * 4));
        } else if flags & SHORT != 0 {
            let offset = half(chunk, header + index * 2)?;

            if offset != NO_SHORT_ENTRY {
                found.push((index, start + usize::from(offset) * 4));
            }
        } else {
            let offset = word(chunk, header + index * 4)?;

            if offset != NO_ENTRY {
                found.push((index, start + usize::try_from(offset).map_err(|_| "A resource entry sits too far into its chunk")?));
            }
        }
    }

    Ok(found)
}

fn remap(chunk: &mut [u8], keys: Option<&[u32]>, values: &[u32]) -> Result<(), String> {
    let mut seen = HashSet::new();

    for (_, at) in slots(chunk)? {
        if seen.insert(at) {
            entry(chunk, at, keys, values)?;
        }
    }

    Ok(())
}

fn detach(chunk: &mut Vec<u8>, slot: usize) -> Result<Option<u32>, String> {
    let Some(at) = slots(chunk)?.into_iter().find(|(index, _)| *index == slot).map(|(_, at)| at) else {
        return Ok(None);
    };
    let entry_flags = half(chunk, at + 2)?;
    let held = if entry_flags & COMPACT != 0 {
        (entry_flags.to_le_bytes()[1] == STRING).then(|| word(chunk, at + VALUE_DATA)).transpose()?
    } else if entry_flags & COMPLEX == 0 {
        let from = at + usize::from(half(chunk, at)?);

        (chunk.get(from + VALUE_KIND) == Some(&STRING)).then(|| word(chunk, from + VALUE_DATA)).transpose()?
    } else {
        None
    };
    let header = usize::from(half(chunk, 2)?);
    let flags = *chunk.get(KIND_FLAGS).ok_or("A resource type chunk is cut short")?;

    if flags & SPARSE != 0 {
        let count = span(chunk, ENTRY_COUNT)?;
        let Some(pair) = (0..count).map(|index| header + index * 4).find(|pair| half(chunk, *pair).is_ok_and(|index| usize::from(index) == slot)) else {
            return Ok(held);
        };
        let (size, start) = (word(chunk, 4)?, word(chunk, ENTRIES_START)?);

        chunk.drain(pair..pair + 4);
        put_word(chunk, 4, size - 4)?;
        put_word(chunk, ENTRY_COUNT, fit(count - 1)?)?;
        put_word(chunk, ENTRIES_START, start - 4)?;
    } else if flags & SHORT != 0 {
        put_half(chunk, header + slot * 2, NO_SHORT_ENTRY)?;
    } else {
        put_word(chunk, header + slot * 4, NO_ENTRY)?;
    }

    Ok(held)
}

impl Package {
    fn parse(chunk: &[u8]) -> Result<Self, String> {
        let header = usize::from(half(chunk, 2)?);
        let head = chunk.get(..header).ok_or("A resource package is cut short")?.to_vec();
        let (type_strings, key_strings) = (span(chunk, TYPE_STRINGS)?, span(chunk, KEY_STRINGS)?);
        let (mut types, mut keys, mut rest) = (None, None, Vec::new());

        for (at, part) in chunks(chunk, header)? {
            if at == type_strings && types.is_none() {
                types = Some(Pool::parse(part)?);
            } else if at == key_strings && keys.is_none() {
                keys = Some(Pool::parse(part)?);
            } else {
                rest.push(part.to_vec());
            }
        }

        Ok(Self {
            id: word(chunk, PACKAGE_ID)?,
            head,
            types: types.ok_or("A resource package names no types")?,
            keys: keys.ok_or("A resource package names no entries")?,
            chunks: rest,
        })
    }

    fn write(&self) -> Result<Vec<u8>, String> {
        let (types, keys) = (self.types.write()?, self.keys.write()?);
        let size = self.head.len() + types.len() + keys.len() + self.chunks.iter().map(Vec::len).sum::<usize>();
        let mut chunk = Vec::with_capacity(size);

        chunk.extend(&self.head);
        put_word(&mut chunk, 4, fit(size)?)?;
        put_word(&mut chunk, TYPE_STRINGS, fit(self.head.len())?)?;
        put_word(&mut chunk, KEY_STRINGS, fit(self.head.len() + types.len())?)?;
        chunk.extend(types);
        chunk.extend(keys);

        for held in &self.chunks {
            chunk.extend(held);
        }

        Ok(chunk)
    }

    fn place(&mut self, chunk: Vec<u8>) {
        let id = chunk.get(KIND_ID).copied();
        let after = self.chunks.iter().rposition(|held| kind_of(held).is_some_and(|(kind, held)| (kind == TYPE || kind == SPEC) && Some(held) == id));

        match after {
            Some(index) => self.chunks.insert(index + 1, chunk),
            None => self.chunks.push(chunk),
        }
    }

    fn blend(&mut self, spec: Vec<u8>) -> Result<(), String> {
        let id = spec.get(KIND_ID).copied();
        let Some(home) = self.chunks.iter_mut().find(|held| kind_of(held).is_some_and(|(kind, held)| kind == SPEC && Some(held) == id)) else {
            self.place(spec);

            return Ok(());
        };
        let (home_header, spec_header) = (usize::from(half(home, 2)?), usize::from(half(&spec, 2)?));
        let (held, incoming) = (span(home, ENTRY_COUNT)?, span(&spec, ENTRY_COUNT)?);

        for index in 0..held.min(incoming) {
            let at = home_header + index * 4;

            let flags = word(home, at)? | word(&spec, spec_header + index * 4)?;

            put_word(home, at, flags)?;
        }

        if incoming > held {
            home.extend(spec.get(spec_header + held * 4..spec_header + incoming * 4).ok_or("A resource type spec is cut short")?);
            let size = fit(home.len())?;

            put_word(home, ENTRY_COUNT, fit(incoming)?)?;
            put_word(home, 4, size)?;
        }

        Ok(())
    }

    fn absorb(&mut self, other: Self, values: &[u32]) -> Result<(), String> {
        let used: HashSet<usize> = other.chunks.iter().filter_map(|held| kind_of(held)).filter(|(kind, _)| *kind == TYPE || *kind == SPEC).map(|(_, id)| usize::from(id)).collect();

        for index in 0..other.types.len() {
            let name = other.types.text(index)?;

            if index >= self.types.len() {
                self.types.push(&name)?;
            } else if used.contains(&(index + 1)) && self.types.text(index)? != name {
                return Err(format!("A split calls resource type {} '{}' where the base calls it '{}'", index + 1, name, self.types.text(index)?));
            }
        }

        let keys = self.keys.absorb(other.keys)?;

        for mut chunk in other.chunks {
            match kind_of(&chunk) {
                Some((SPEC, _)) => self.blend(chunk)?,
                Some((TYPE, _)) => {
                    remap(&mut chunk, Some(&keys), values)?;
                    self.place(chunk);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl Table {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if half(bytes, 0)? != TABLE {
            return Err("resources.arsc is not a resource table".to_string());
        }

        let (mut values, mut parts) = (None, Vec::new());

        for (_, chunk) in chunks(bytes, usize::from(half(bytes, 2)?))? {
            match half(chunk, 0)? {
                pool::CHUNK if values.is_none() => values = Some(Pool::parse(chunk)?),
                PACKAGE => parts.push(Part::Package(Package::parse(chunk)?)),
                _ => parts.push(Part::Other(chunk.to_vec())),
            }
        }

        Ok(Self { values: values.ok_or("resources.arsc holds no strings")?, parts })
    }

    pub(super) fn absorb(&mut self, other: Self) -> Result<(), String> {
        let values = self.values.absorb(other.values)?;

        for part in other.parts {
            let Part::Package(mut package) = part else {
                continue;
            };
            let home = self.parts.iter_mut().find_map(|held| match held {
                Part::Package(held) if held.id == package.id => Some(held),
                _ => None,
            });

            if let Some(home) = home {
                home.absorb(package, &values)?;

                continue;
            }

            for chunk in &mut package.chunks {
                if kind_of(chunk).is_some_and(|(kind, _)| kind == TYPE) {
                    remap(chunk, None, &values)?;
                }
            }

            self.parts.push(Part::Package(package));
        }

        Ok(())
    }

    pub(super) fn forget(&mut self, resource: u32) -> Result<Vec<String>, String> {
        let [slot_low, slot_high, kind, package] = resource.to_le_bytes();
        let slot = usize::from(u16::from_le_bytes([slot_low, slot_high]));
        let mut texts = Vec::new();

        for part in &mut self.parts {
            let Part::Package(held) = part else {
                continue;
            };

            if held.id != u32::from(package) {
                continue;
            }

            for chunk in &mut held.chunks {
                if kind_of(chunk) != Some((TYPE, kind)) {
                    continue;
                }

                if let Some(index) = detach(chunk, slot)? {
                    texts.push(self.values.text(usize::try_from(index).map_err(|_| "A resource points at a string outside its pool")?)?);
                }
            }
        }

        Ok(texts)
    }

    pub(super) fn write(&self) -> Result<Vec<u8>, String> {
        let mut body = self.values.write()?;
        let mut packages = 0u32;

        for part in &self.parts {
            match part {
                Part::Package(held) => {
                    body.extend(held.write()?);
                    packages += 1;
                }
                Part::Other(held) => body.extend(held),
            }
        }

        let mut table = Vec::with_capacity(body.len() + usize::from(TABLE_HEADER));

        table.extend(TABLE.to_le_bytes());
        table.extend(TABLE_HEADER.to_le_bytes());
        table.extend(fit(body.len() + usize::from(TABLE_HEADER))?.to_le_bytes());
        table.extend(packages.to_le_bytes());
        table.extend(body);

        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKAGE_HEADER: usize = 288;
    const APP: u32 = 0x7F;
    const DENSITY: u32 = 0x0100;
    const LOCALE: u32 = 0x0004;

    fn words(fields: &[u32]) -> Vec<u8> {
        fields.iter().flat_map(|field| field.to_le_bytes()).collect()
    }

    fn spec(id: u8, flags: &[u32]) -> Vec<u8> {
        let mut chunk = Vec::new();

        chunk.extend(SPEC.to_le_bytes());
        chunk.extend(16u16.to_le_bytes());
        chunk.extend(words(&[16 + 4 * flags.len() as u32]));
        chunk.extend([id, 0, 0, 0]);
        chunk.extend(words(&[flags.len() as u32]));
        chunk.extend(words(flags));
        chunk
    }

    // One string entry per slot: (key index, value string index), or nothing.
    fn kind(id: u8, sparse: bool, slots: &[Option<(u32, u32)>]) -> Vec<u8> {
        let held: Vec<(usize, (u32, u32))> = slots.iter().enumerate().filter_map(|(index, slot)| slot.map(|slot| (index, slot))).collect();
        let mut offsets = Vec::new();
        let mut entries = Vec::new();

        for (index, slot) in slots.iter().enumerate() {
            match (slot, sparse) {
                (Some((key, text)), _) => {
                    let at = entries.len() as u32;

                    if sparse {
                        offsets.extend((index as u16).to_le_bytes());
                        offsets.extend(((at / 4) as u16).to_le_bytes());
                    } else {
                        offsets.extend(words(&[at]));
                    }

                    entries.extend(8u16.to_le_bytes());
                    entries.extend(0u16.to_le_bytes());
                    entries.extend(words(&[*key]));
                    entries.extend([8, 0, 0, STRING]);
                    entries.extend(words(&[*text]));
                }
                (None, false) => offsets.extend(words(&[NO_ENTRY])),
                (None, true) => {}
            }
        }

        let header = 28u32;
        let count = if sparse { held.len() } else { slots.len() } as u32;
        let mut chunk = Vec::new();

        chunk.extend(TYPE.to_le_bytes());
        chunk.extend((header as u16).to_le_bytes());
        chunk.extend(words(&[header + offsets.len() as u32 + entries.len() as u32]));
        chunk.extend([id, if sparse { SPARSE } else { 0 }, 0, 0]);
        chunk.extend(words(&[count, header + offsets.len() as u32, 8, 0]));
        chunk.extend(offsets);
        chunk.extend(entries);
        chunk
    }

    fn table(values: &[&str], types: &[&str], keys: &[&str], chunks: Vec<Vec<u8>>) -> Table {
        let mut head = vec![0; PACKAGE_HEADER];

        head[..2].copy_from_slice(&PACKAGE.to_le_bytes());
        head[2..4].copy_from_slice(&(PACKAGE_HEADER as u16).to_le_bytes());
        head[PACKAGE_ID..PACKAGE_ID + 4].copy_from_slice(&APP.to_le_bytes());

        let package = Package { head, id: APP, types: Pool::of(false, types), keys: Pool::of(true, keys), chunks };
        let built = Table { values: Pool::of(true, values), parts: vec![Part::Package(package)] };

        // through bytes, so the layout the writer produces is what gets merged
        Table::parse(&built.write().unwrap()).unwrap()
    }

    fn package(table: &Table) -> &Package {
        table.parts.iter().find_map(|part| if let Part::Package(held) = part { Some(held) } else { None }).unwrap()
    }

    fn entries(table: &Table, chunk: &[u8]) -> Vec<(usize, String, String)> {
        slots(chunk)
            .unwrap()
            .into_iter()
            .map(|(slot, at)| (slot, package(table).keys.text(span(chunk, at + 4).unwrap()).unwrap(), table.values.text(span(chunk, at + 8 + VALUE_DATA).unwrap()).unwrap()))
            .collect()
    }

    #[test]
    fn a_split_lands_in_the_base_with_its_strings_and_keys_remapped() {
        let mut base = table(
            &["res/drawable/a.xml", "Cats"],
            &["drawable", "string"],
            &["a", "app_name"],
            vec![spec(1, &[0, 0]), kind(1, false, &[Some((0, 0)), None]), spec(2, &[0]), kind(2, false, &[Some((1, 1))])],
        );
        // the split numbers its own strings and keys from zero, and pads the type it does not hold
        let split = table(
            &["res/drawable-xxhdpi/b.png", "res/drawable-xxhdpi/a.png"],
            &["drawable", "?2"],
            &["b", "a"],
            vec![spec(1, &[DENSITY, DENSITY | LOCALE]), kind(1, false, &[Some((1, 1)), Some((0, 0))])],
        );

        base.absorb(split).unwrap();

        let merged = Table::parse(&base.write().unwrap()).unwrap();
        let held = &package(&merged).chunks;
        let kinds: Vec<_> = held.iter().map(|chunk| kind_of(chunk).unwrap()).collect();

        assert_eq!(kinds, [(SPEC, 1), (TYPE, 1), (TYPE, 1), (SPEC, 2), (TYPE, 2)], "the split's chunk sits with its own type");
        assert_eq!(entries(&merged, &held[1]), [(0, "a".to_string(), "res/drawable/a.xml".to_string())]);
        assert_eq!(
            entries(&merged, &held[2]),
            [(0, "a".to_string(), "res/drawable-xxhdpi/a.png".to_string()), (1, "b".to_string(), "res/drawable-xxhdpi/b.png".to_string())]
        );
        assert_eq!(entries(&merged, &held[4]), [(0, "app_name".to_string(), "Cats".to_string())]);
        assert_eq!((word(&held[0], 16).unwrap(), word(&held[0], 20).unwrap()), (DENSITY, DENSITY | LOCALE), "spec flags are the union");
        assert_eq!(package(&merged).types.text(1).unwrap(), "string");
    }

    #[test]
    fn a_type_the_split_really_holds_must_carry_the_base_name() {
        let mut base = table(&["x"], &["drawable"], &["a"], vec![spec(1, &[0]), kind(1, false, &[Some((0, 0))])]);
        let split = table(&["y"], &["layout"], &["a"], vec![spec(1, &[0]), kind(1, false, &[Some((0, 0))])]);

        assert!(base.absorb(split).is_err());
    }

    #[test]
    fn forgetting_an_entry_hands_back_its_file_in_both_layouts() {
        for sparse in [false, true] {
            let mut held = table(
                &["res/xml/keep.xml", "res/xml/splits0.xml"],
                &["xml"],
                &["keep", "splits0"],
                vec![spec(1, &[0, 0]), kind(1, sparse, &[Some((0, 0)), Some((1, 1))])],
            );

            assert_eq!(held.forget(0x7F01_0001).unwrap(), ["res/xml/splits0.xml"]);

            let after = Table::parse(&held.write().unwrap()).unwrap();

            assert_eq!(entries(&after, &package(&after).chunks[1]), [(0, "keep".to_string(), "res/xml/keep.xml".to_string())]);
            assert!(held.forget(0x7F01_0001).unwrap().is_empty());
        }
    }
}
