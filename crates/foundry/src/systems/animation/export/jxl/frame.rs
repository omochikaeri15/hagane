use super::{bits::BitWriter, lossless};

pub(super) const TRANSFORMED: u8 = 1;
pub(super) const KEY: u8 = 0;
pub(super) const CLEAR: u8 = 2;
pub(super) const PAINT: u8 = 4;
const ROLE: u8 = 6;
const SIZE_CHOICES: [(u32, u32); 4] = [(9, 1), (13, 1), (18, 1), (30, 1)];
const PLACE_CHOICES: [(u32, u32); 4] = [(8, 0), (11, 256), (14, 2304), (30, 18688)];
const ENTRY_CHOICES: [(u32, u32); 4] = [(10, 0), (14, 1024), (22, 17408), (30, 4_211_712)];

pub(super) fn sections(parts: Vec<Vec<u8>>) -> Vec<u8> {
    let mut writer = BitWriter::with_capacity(parts.iter().map(Vec::len).sum::<usize>() + parts.len() * 4 + 8);
    for part in &parts {
        writer.select(part.len() as u32, ENTRY_CHOICES);
    }
    for part in &parts {
        writer.extend(part);
    }
    writer.finish()
}

pub(super) fn file_header(writer: &mut BitWriter, width: u32, height: u32, ticks_per_second: u32) {
    writer.write(16, 0x0aff);
    writer.flag(false);
    writer.select(height.max(1), SIZE_CHOICES);
    writer.write(3, 0);
    writer.select(width.max(1), SIZE_CHOICES);
    writer.flag(false);
    writer.flag(true);
    writer.write(3, 0);
    writer.flag(false);
    writer.flag(false);
    writer.flag(true);
    writer.select(ticks_per_second.max(1), [(0, 100), (0, 1000), (10, 1), (30, 1)]);
    writer.select(1, [(0, 1), (0, 1001), (8, 1), (10, 1)]);
    writer.select(0, [(0, 0), (3, 0), (16, 0), (32, 0)]);
    writer.flag(false);
    writer.flag(false);
    writer.write(2, 0);
    writer.flag(true);
    writer.write(2, 1);
    writer.flag(true);
    writer.flag(false);
    writer.flag(true);
    writer.flag(true);
    writer.write(2, 0);
    writer.flag(true);
    writer.pad();
}

pub(super) struct Header {
    pub(super) kind: u8,
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) ticks: u32,
    pub(super) save: bool,
    pub(super) last: bool,
}

pub(super) fn frame_header(writer: &mut BitWriter, canvas: (u32, u32), header: &Header) {
    let modular = header.kind & TRANSFORMED == 0;
    let role = header.kind & ROLE;
    let partial = header.x > 0 || header.y > 0 || header.x + header.width < canvas.0 || header.y + header.height < canvas.1;
    let custom = partial || header.width != canvas.0 || header.height != canvas.1;
    writer.flag(false);
    writer.write(2, 0);
    writer.flag(modular);
    if modular {
        writer.write(2, 0);
        writer.flag(false);
    } else {
        writer.write(2, 2);
        writer.write(8, 111);
        writer.flag(true);
        writer.write(6, 0);
    }
    writer.write(2, 0);
    writer.write(2, 0);
    if modular {
        writer.write(2, u64::from(lossless::GROUP_SHIFT));
    }
    writer.write(2, 0);
    writer.flag(custom);
    if custom {
        writer.select(header.x * 2, PLACE_CHOICES);
        writer.select(header.y * 2, PLACE_CHOICES);
        writer.select(header.width, PLACE_CHOICES);
        writer.select(header.height, PLACE_CHOICES);
    }
    for channel in 0..2 {
        match role {
            CLEAR if channel == 0 => {
                writer.write(2, 1);
                writer.write(2, 1);
            }
            CLEAR => {
                writer.write(2, 3);
                writer.write(2, 1);
                writer.flag(false);
                writer.write(2, 1);
            }
            PAINT => {
                writer.write(2, 2);
                writer.write(2, 0);
                writer.flag(false);
                writer.write(2, 1);
            }
            _ => {
                writer.write(2, 0);
                if partial {
                    writer.write(2, 0);
                }
            }
        }
    }
    writer.select(header.ticks, [(0, 0), (0, 1), (8, 0), (32, 0)]);
    writer.flag(header.last);
    if !header.last {
        writer.write(2, u64::from(header.save));
        if (header.save || header.ticks == 0) && role == KEY && !partial {
            writer.flag(false);
        }
    }
    writer.write(2, 0);
    writer.flag(false);
    writer.flag(false);
    writer.write(2, 0);
    writer.write(2, 0);
    writer.write(2, 0);
    writer.flag(false);
    writer.pad();
}
