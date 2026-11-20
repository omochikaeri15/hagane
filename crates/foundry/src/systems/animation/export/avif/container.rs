use std::collections::HashMap;

use super::{Placed, header::level};

const ALPHA_KIND: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0";
const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];
const FOREVER: u64 = u64::MAX;

pub(super) struct Sample {
    pub color: (u32, u32),
    pub alpha: (u32, u32),
    pub ticks: u32,
    pub key: bool,
}

struct Sheet<'a> {
    width: u32,
    height: u32,
    rate: u32,
    identity: bool,
    samples: &'a [Sample],
    base: u32,
}

fn boxed(out: &mut Vec<u8>, name: &[u8; 4], fill: impl FnOnce(&mut Vec<u8>)) {
    let start = out.len();
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(name);
    fill(out);
    let size = (out.len() - start) as u32;
    out[start..start + 4].copy_from_slice(&size.to_be_bytes());
}

fn full(out: &mut Vec<u8>, name: &[u8; 4], version: u8, flags: u32, fill: impl FnOnce(&mut Vec<u8>)) {
    boxed(out, name, |out| {
        out.extend_from_slice(&((u32::from(version) << 24) | flags).to_be_bytes());
        fill(out);
    });
}

fn long(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn short(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn huge(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn handler(out: &mut Vec<u8>, kind: &[u8; 4]) {
    full(out, b"hdlr", 0, 0, |out| {
        long(out, 0);
        out.extend_from_slice(kind);
        out.extend_from_slice(&[0; 13]);
    });
}

fn codec(out: &mut Vec<u8>, sheet: &Sheet<'_>, color: bool) {
    boxed(out, b"av1C", |out| {
        let level = level(sheet.width, sheet.height);
        out.extend_from_slice(&[0x81, (u8::from(color) << 5) | level, if color { 0 } else { 0x1C }, 0]);
    });
}

fn shade(out: &mut Vec<u8>, sheet: &Sheet<'_>) {
    boxed(out, b"colr", |out| {
        out.extend_from_slice(b"nclx");
        short(out, 1);
        short(out, 13);
        short(out, if sheet.identity { 0 } else { 6 });
        out.push(0x80);
    });
}

fn items(out: &mut Vec<u8>, sheet: &Sheet<'_>) {
    let first = &sheet.samples[0];
    full(out, b"meta", 0, 0, |out| {
        handler(out, b"pict");
        full(out, b"pitm", 0, 0, |out| short(out, 1));
        full(out, b"iloc", 0, 0, |out| {
            out.extend_from_slice(&[0x44, 0]);
            short(out, 2);
            for (id, (offset, length)) in [(1, first.color), (2, first.alpha)] {
                short(out, id);
                short(out, 0);
                short(out, 1);
                long(out, sheet.base + offset);
                long(out, length);
            }
        });
        full(out, b"iinf", 0, 0, |out| {
            short(out, 2);
            for (id, name) in [(1, &b"Color\0"[..]), (2, &b"Alpha\0"[..])] {
                full(out, b"infe", 2, 0, |out| {
                    short(out, id);
                    short(out, 0);
                    out.extend_from_slice(b"av01");
                    out.extend_from_slice(name);
                });
            }
        });
        full(out, b"iref", 0, 0, |out| {
            boxed(out, b"auxl", |out| {
                short(out, 2);
                short(out, 1);
                short(out, 1);
            });
        });
        boxed(out, b"iprp", |out| {
            boxed(out, b"ipco", |out| {
                full(out, b"ispe", 0, 0, |out| {
                    long(out, sheet.width);
                    long(out, sheet.height);
                });
                full(out, b"pixi", 0, 0, |out| out.extend_from_slice(&[3, 8, 8, 8]));
                codec(out, sheet, true);
                shade(out, sheet);
                full(out, b"pixi", 0, 0, |out| out.extend_from_slice(&[1, 8]));
                codec(out, sheet, false);
                full(out, b"auxC", 0, 0, |out| out.extend_from_slice(ALPHA_KIND));
            });
            full(out, b"ipma", 0, 0, |out| {
                long(out, 2);
                short(out, 1);
                out.extend_from_slice(&[4, 1, 2, 0x83, 4]);
                short(out, 2);
                out.extend_from_slice(&[4, 1, 5, 0x86, 7]);
            });
        });
    });
}

fn track(out: &mut Vec<u8>, sheet: &Sheet<'_>, color: bool) {
    let length: u64 = sheet.samples.iter().map(|sample| u64::from(sample.ticks)).sum();
    boxed(out, b"trak", |out| {
        full(out, b"tkhd", 1, 1, |out| {
            huge(out, 0);
            huge(out, 0);
            long(out, if color { 1 } else { 2 });
            long(out, 0);
            huge(out, FOREVER);
            out.extend_from_slice(&[0; 16]);
            MATRIX.iter().for_each(|&cell| long(out, cell));
            long(out, sheet.width << 16);
            long(out, sheet.height << 16);
        });
        if !color {
            boxed(out, b"tref", |out| boxed(out, b"auxl", |out| long(out, 1)));
        }
        boxed(out, b"edts", |out| {
            full(out, b"elst", 1, 1, |out| {
                long(out, 1);
                huge(out, length);
                huge(out, 0);
                short(out, 1);
                short(out, 0);
            });
        });
        boxed(out, b"mdia", |out| {
            full(out, b"mdhd", 1, 0, |out| {
                huge(out, 0);
                huge(out, 0);
                long(out, sheet.rate);
                huge(out, length);
                short(out, 0x55C4);
                short(out, 0);
            });
            handler(out, if color { b"pict" } else { b"auxv" });
            boxed(out, b"minf", |out| {
                full(out, b"vmhd", 0, 1, |out| out.extend_from_slice(&[0; 8]));
                boxed(out, b"dinf", |out| {
                    full(out, b"dref", 0, 0, |out| {
                        long(out, 1);
                        full(out, b"url ", 0, 1, |_| {});
                    });
                });
                boxed(out, b"stbl", |out| table(out, sheet, color));
            });
        });
    });
}

fn table(out: &mut Vec<u8>, sheet: &Sheet<'_>, color: bool) {
    let count = sheet.samples.len() as u32;
    let place = |sample: &Sample| if color { sample.color } else { sample.alpha };
    full(out, b"stsd", 0, 0, |out| {
        long(out, 1);
        boxed(out, b"av01", |out| {
            out.extend_from_slice(&[0; 6]);
            short(out, 1);
            out.extend_from_slice(&[0; 16]);
            short(out, sheet.width as u16);
            short(out, sheet.height as u16);
            long(out, 0x0048_0000);
            long(out, 0x0048_0000);
            long(out, 0);
            short(out, 1);
            out.extend_from_slice(&[0; 32]);
            short(out, 0x0018);
            short(out, 0xFFFF);
            codec(out, sheet, color);
            if color {
                shade(out, sheet);
            }
            full(out, b"ccst", 0, 0, |out| out.extend_from_slice(&[0x7C, 0, 0, 0]));
            if !color {
                full(out, b"auxi", 0, 0, |out| out.extend_from_slice(ALPHA_KIND));
            }
        });
    });
    full(out, b"stts", 0, 0, |out| {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for sample in sheet.samples {
            if let Some((count, _)) = runs.last_mut().filter(|(_, ticks)| *ticks == sample.ticks) {
                *count += 1;
            } else {
                runs.push((1, sample.ticks));
            }
        }
        long(out, runs.len() as u32);
        for (count, ticks) in runs {
            long(out, count);
            long(out, ticks);
        }
    });
    full(out, b"stsc", 0, 0, |out| {
        long(out, 1);
        long(out, 1);
        long(out, 1);
        long(out, 1);
    });
    full(out, b"stsz", 0, 0, |out| {
        long(out, 0);
        long(out, count);
        sheet.samples.iter().for_each(|sample| long(out, place(sample).1));
    });
    full(out, b"stco", 0, 0, |out| {
        long(out, count);
        sheet.samples.iter().for_each(|sample| long(out, sheet.base + place(sample).0));
    });
    if sheet.samples.iter().any(|sample| !sample.key) {
        full(out, b"stss", 0, 0, |out| {
            long(out, sheet.samples.iter().filter(|sample| sample.key).count() as u32);
            for (index, _) in sheet.samples.iter().enumerate().filter(|(_, sample)| sample.key) {
                long(out, index as u32 + 1);
            }
        });
    }
}

fn head(sheet: &Sheet<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    boxed(&mut out, b"ftyp", |out| {
        out.extend_from_slice(b"avis");
        long(out, 0);
        out.extend_from_slice(b"avif");
        if sheet.samples.iter().all(|sample| sample.key) {
            out.extend_from_slice(b"avio");
        }
        out.extend_from_slice(b"avismsf1iso8mif1miafMA1A");
    });
    items(&mut out, sheet);
    boxed(&mut out, b"moov", |out| {
        full(out, b"mvhd", 1, 0, |out| {
            huge(out, 0);
            huge(out, 0);
            long(out, sheet.rate);
            huge(out, FOREVER);
            long(out, 0x0001_0000);
            short(out, 0x0100);
            out.extend_from_slice(&[0; 10]);
            MATRIX.iter().for_each(|&cell| long(out, cell));
            out.extend_from_slice(&[0; 24]);
            long(out, 3);
        });
        track(out, sheet, true);
        track(out, sheet, false);
    });
    out
}

pub(super) fn split(payload: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (tag, rest) = payload.split_first()?;
    let (size, rest) = rest.split_first_chunk::<4>()?;
    let size = u32::from_le_bytes(*size) as usize;
    (size <= rest.len()).then(|| (*tag, &rest[..size], &rest[size..]))
}

pub(super) fn join(identity: bool, key: bool, color: &[u8], alpha: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + color.len() + alpha.len());
    out.push(u8::from(identity) | (u8::from(key) << 1));
    out.extend_from_slice(&(color.len() as u32).to_le_bytes());
    out.extend_from_slice(color);
    out.extend_from_slice(alpha);
    out
}

pub(super) fn assemble(width: u32, height: u32, rate: u32, frames: &[Placed<'_>]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut seen: HashMap<&[u8], (u32, u32)> = HashMap::new();
    let mut keys: HashMap<&[u8], usize> = HashMap::new();
    let mut samples: Vec<Sample> = Vec::with_capacity(frames.len());
    let mut identity = false;
    let mut mirror: Option<usize> = None;
    for (index, frame) in frames.iter().enumerate() {
        let Some((tag, color, alpha)) = split(frame.payload) else {
            return Vec::new();
        };
        identity = tag & 1 != 0;
        let key = tag & 2 != 0;
        mirror = if key {
            keys.get(frame.payload).copied()
        } else {
            mirror.map(|at| at + 1).filter(|&at| at < index && frames[at].payload == frame.payload && samples.get(at).is_some_and(|sample| !sample.key))
        };
        let mut places = mirror.and_then(|at| samples.get(at)).map_or([(0, 0); 2], |sample| [sample.color, sample.alpha]);
        if mirror.is_none() {
            for (place, piece) in places.iter_mut().zip([color, alpha]) {
                *place = match seen.get(piece).filter(|_| key) {
                    Some(&known) => known,
                    None => {
                        let fresh = (data.len() as u32, piece.len() as u32);
                        data.extend_from_slice(piece);
                        if key {
                            seen.insert(piece, fresh);
                        }
                        fresh
                    }
                };
            }
        }
        if key {
            keys.entry(frame.payload).or_insert(index);
        }
        samples.push(Sample { color: places[0], alpha: places[1], ticks: frame.ticks, key });
    }
    if samples.is_empty() {
        return Vec::new();
    }
    let mut sheet = Sheet { width, height, rate, identity, samples: &samples, base: 0 };
    sheet.base = head(&sheet).len() as u32 + 8;
    let mut out = head(&sheet);
    out.extend_from_slice(&(data.len() as u32 + 8).to_be_bytes());
    out.extend_from_slice(b"mdat");
    out.extend_from_slice(&data);
    out
}
