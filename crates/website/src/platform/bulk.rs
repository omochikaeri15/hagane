use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::{Error, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const PREPARED: usize = 1 << 20;
const FLOAT: usize = size_of::<f32>();

struct Raw<'a>(&'a [u8]);

impl Serialize for Raw<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

struct Owned<T>(T);

struct Whole<T>(PhantomData<T>);

impl<'de, T: From<Vec<u8>> + for<'a> From<&'a [u8]>> Visitor<'de> for Whole<T> {
    type Value = Owned<T>;

    fn expecting(&self, out: &mut fmt::Formatter) -> fmt::Result {
        out.write_str("a run of bytes")
    }

    fn visit_bytes<E: Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
        Ok(Owned(T::from(bytes)))
    }

    fn visit_byte_buf<E: Error>(self, bytes: Vec<u8>) -> Result<Self::Value, E> {
        Ok(Owned(T::from(bytes)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(PREPARED));

        while let Some(byte) = seq.next_element()? {
            bytes.push(byte);
        }

        Ok(Owned(T::from(bytes)))
    }
}

impl<'de, T: From<Vec<u8>> + for<'a> From<&'a [u8]>> Deserialize<'de> for Owned<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_bytes(Whole(PhantomData))
    }
}

pub mod bytes {
    use super::*;

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        Raw(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        Owned::deserialize(deserializer).map(|held| held.0)
    }
}

pub mod shared {
    use super::*;

    pub fn serialize<S: Serializer>(bytes: &Arc<[u8]>, serializer: S) -> Result<S::Ok, S::Error> {
        Raw(bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Arc<[u8]>, D::Error> {
        Owned::deserialize(deserializer).map(|held| held.0)
    }
}

pub mod files {
    use super::*;

    type Files = Vec<(Box<str>, Arc<[u8]>)>;

    pub fn serialize<S: Serializer>(files: &Files, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(files.len()))?;

        for (name, bytes) in files {
            seq.serialize_element(&(name, Raw(bytes)))?;
        }

        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Files, D::Error> {
        Vec::<(Box<str>, Owned<Arc<[u8]>>)>::deserialize(deserializer).map(|files| files.into_iter().map(|(name, held)| (name, held.0)).collect())
    }
}

pub mod outcome {
    use super::*;

    pub fn serialize<S: Serializer>(outcome: &Result<Vec<u8>, String>, serializer: S) -> Result<S::Ok, S::Error> {
        outcome.as_ref().map(|bytes| Raw(bytes)).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Result<Vec<u8>, String>, D::Error> {
        Result::<Owned<Vec<u8>>, String>::deserialize(deserializer).map(|outcome| outcome.map(|held| held.0))
    }
}

pub mod floats {
    use super::*;

    pub fn serialize<S: Serializer>(samples: &[f32], serializer: S) -> Result<S::Ok, S::Error> {
        if cfg!(target_endian = "little") {
            return Raw(bytemuck::cast_slice(samples)).serialize(serializer);
        }

        let bytes: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();

        Raw(&bytes).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<f32>, D::Error> {
        let bytes = Owned::<Vec<u8>>::deserialize(deserializer)?.0;

        let (samples, torn) = bytes.as_chunks::<FLOAT>();

        if !torn.is_empty() {
            return Err(D::Error::custom("a run of samples ends inside one"));
        }

        Ok(samples.iter().map(|sample| f32::from_le_bytes(*sample)).collect())
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Plain {
        pixels: Vec<u8>,
        files: Vec<(Box<str>, Vec<u8>)>,
        outcome: Result<Vec<u8>, String>,
        refused: Result<Vec<u8>, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Bulk {
        #[serde(with = "bytes")]
        pixels: Vec<u8>,
        #[serde(with = "files")]
        files: Vec<(Box<str>, Arc<[u8]>)>,
        #[serde(with = "outcome")]
        outcome: Result<Vec<u8>, String>,
        #[serde(with = "outcome")]
        refused: Result<Vec<u8>, String>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Sound {
        #[serde(with = "floats")]
        samples: Vec<f32>,
        #[serde(with = "shared")]
        model: Arc<[u8]>,
    }

    // a block of bytes has to land on the wire exactly as the byte-by-byte form did, or a stored battle stops opening
    #[test]
    fn bytes_sent_whole_read_the_same_as_bytes_sent_one_by_one() {
        let pixels: Vec<u8> = (0..70_000u32).map(|at| (at * 31 + at / 7) as u8).collect();
        let plain = Plain {
            pixels: pixels.clone(),
            files: vec![(Box::from("unit001.csv"), pixels[..300].to_vec()), (Box::from("empty.tsv"), Vec::new())],
            outcome: Ok(pixels[..9].to_vec()),
            refused: Err("too large".to_owned()),
        };
        let bulk = Bulk {
            pixels: pixels.clone(),
            files: plain.files.iter().map(|(name, bytes)| (name.clone(), Arc::from(bytes.as_slice()))).collect(),
            outcome: plain.outcome.clone(),
            refused: plain.refused.clone(),
        };
        let wire = postcard::to_allocvec(&bulk).unwrap();

        assert_eq!(wire, postcard::to_allocvec(&plain).unwrap());
        assert_eq!(postcard::from_bytes::<Bulk>(&wire).unwrap(), bulk);
        assert_eq!(postcard::from_bytes::<Plain>(&wire).unwrap(), plain);
    }

    #[test]
    fn samples_and_shared_bytes_come_back_as_they_went() {
        let sound = Sound { samples: vec![0.0, -1.0, 0.25, f32::MAX, f32::MIN_POSITIVE], model: Arc::from(&b"model"[..]) };
        let wire = postcard::to_allocvec(&sound).unwrap();

        assert_eq!(postcard::from_bytes::<Sound>(&wire).unwrap(), sound);
        assert!(postcard::from_bytes::<Sound>(&wire[..wire.len() - 7]).is_err());
    }
}
