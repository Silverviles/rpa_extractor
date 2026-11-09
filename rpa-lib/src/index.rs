use crate::error::RpaError;
use crate::header::{RpaHeader, RpaVersion};
use flate2::read::ZlibDecoder;
use serde_pickle::value::{HashableValue, Value};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub offset: u64,
    pub length: u64,
    pub prefix: Vec<u8>,
}

pub fn parse_index(
    reader: &mut (impl Read + Seek),
    header: &RpaHeader,
) -> Result<HashMap<String, IndexEntry>, RpaError> {
    reader
        .seek(SeekFrom::Start(header.index_offset))
        .map_err(|e| RpaError::Io(e))?;

    let mut compressed = Vec::new();
    reader
        .read_to_end(&mut compressed)
        .map_err(|e| RpaError::Io(e))?;

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| RpaError::DecompressionFailed(e.to_string()))?;

    let pickle_value: Value =
        serde_pickle::value_from_slice(&decompressed, serde_pickle::DeOptions::default())
            .map_err(|e| RpaError::PickleFailed(e.to_string()))?;

    let dict = match pickle_value {
        Value::Dict(d) => d,
        _ => {
            return Err(RpaError::PickleFailed(
                "Index is not a dictionary".to_string(),
            ))
        }
    };

    let mut entries = HashMap::new();

    for (key, value) in dict {
        let filename = match key {
            HashableValue::String(s) => s.replace('\0', ""),
            HashableValue::Bytes(b) => String::from_utf8_lossy(&b).replace('\0', ""),
            _ => continue,
        };

        let tuple_list = match value {
            Value::List(l) => l,
            _ => continue,
        };

        if tuple_list.is_empty() {
            continue;
        }

        // Take the first tuple
        let tuple = match &tuple_list[0] {
            Value::Tuple(t) => t,
            _ => continue,
        };

        if tuple.len() < 2 {
            continue;
        }

        let raw_offset = extract_int(&tuple[0])
            .ok_or_else(|| RpaError::PickleFailed(format!("Invalid offset for {}", filename)))?;
        let raw_length = extract_int(&tuple[1])
            .ok_or_else(|| RpaError::PickleFailed(format!("Invalid length for {}", filename)))?;

        let prefix = if tuple.len() > 2 {
            match &tuple[2] {
                Value::Bytes(b) => b.clone(),
                Value::String(s) => s.as_bytes().to_vec(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let (offset, length) = match header.version {
            RpaVersion::V2_0 => (raw_offset as u64, raw_length as u64),
            RpaVersion::V3_0 | RpaVersion::V3_2 => {
                let offset = (raw_offset as u64) ^ header.key;
                let length = (raw_length as u64) ^ header.key;
                (offset, length)
            }
        };

        entries.insert(
            filename,
            IndexEntry {
                offset,
                length,
                prefix,
            },
        );
    }

    Ok(entries)
}

fn extract_int(value: &Value) -> Option<i64> {
    match value {
        Value::I64(n) => Some(*n),
        Value::F64(n) => Some(*n as i64),
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        _ => None,
    }
}
