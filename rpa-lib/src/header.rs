use crate::error::RpaError;
use std::io::{BufRead, BufReader, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpaVersion {
    V2_0,
    V3_0,
    V3_2,
}

impl RpaVersion {
    pub fn magic(&self) -> &'static str {
        match self {
            RpaVersion::V2_0 => "RPA-2.0",
            RpaVersion::V3_0 => "RPA-3.0",
            RpaVersion::V3_2 => "RPA-3.2",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpaHeader {
    pub version: RpaVersion,
    pub index_offset: u64,
    pub key: u64,
}

pub fn parse_header(reader: &mut impl Read) -> Result<RpaHeader, RpaError> {
    let mut buf_reader = BufReader::new(reader);
    let mut first_line = String::new();
    buf_reader
        .read_line(&mut first_line)
        .map_err(|e| RpaError::InvalidHeader(format!("Failed to read header line: {}", e)))?;

    let first_line = first_line.trim_end_matches('\n').trim_end_matches('\r');
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.is_empty() {
        return Err(RpaError::InvalidHeader("Empty header".to_string()));
    }

    let version = match parts[0] {
        "RPA-2.0" => RpaVersion::V2_0,
        "RPA-3.0" => RpaVersion::V3_0,
        "RPA-3.2" => RpaVersion::V3_2,
        other => return Err(RpaError::UnsupportedVersion(other.to_string())),
    };

    if parts.len() < 2 {
        return Err(RpaError::InvalidHeader(
            "Missing index offset".to_string(),
        ));
    }

    let index_offset = u64::from_str_radix(parts[1], 16)
        .map_err(|e| RpaError::InvalidHeader(format!("Invalid index offset: {}", e)))?;

    let key = match version {
        RpaVersion::V2_0 => 0,
        RpaVersion::V3_0 => {
            if parts.len() < 3 {
                return Err(RpaError::InvalidHeader("Missing XOR key".to_string()));
            }
            u64::from_str_radix(parts[2], 16)
                .map_err(|e| RpaError::InvalidHeader(format!("Invalid XOR key: {}", e)))?
        }
        RpaVersion::V3_2 => {
            if parts.len() < 3 {
                return Err(RpaError::InvalidHeader("Missing XOR key(s)".to_string()));
            }
            let mut combined_key: u64 = 0;
            for part in &parts[2..] {
                let k = u64::from_str_radix(part, 16)
                    .map_err(|e| RpaError::InvalidHeader(format!("Invalid XOR key: {}", e)))?;
                combined_key ^= k;
            }
            combined_key
        }
    };

    Ok(RpaHeader {
        version,
        index_offset,
        key,
    })
}

pub fn write_header(version: RpaVersion, index_offset: u64, key: u64) -> String {
    match version {
        RpaVersion::V2_0 => format!("RPA-2.0 {:016x}\n", index_offset),
        RpaVersion::V3_0 => format!("RPA-3.0 {:016x} {:08x}\n", index_offset, key),
        RpaVersion::V3_2 => format!("RPA-3.2 {:016x} {:08x}\n", index_offset, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_rpa2() {
        let header = b"RPA-2.0 00000000000012ab\n";
        let mut cursor = Cursor::new(header);
        let h = parse_header(&mut cursor).unwrap();
        assert_eq!(h.version, RpaVersion::V2_0);
        assert_eq!(h.index_offset, 0x12ab);
        assert_eq!(h.key, 0);
    }

    #[test]
    fn test_parse_rpa3() {
        let header = b"RPA-3.0 00000000000012ab 42424242\n";
        let mut cursor = Cursor::new(header);
        let h = parse_header(&mut cursor).unwrap();
        assert_eq!(h.version, RpaVersion::V3_0);
        assert_eq!(h.index_offset, 0x12ab);
        assert_eq!(h.key, 0x42424242);
    }

    #[test]
    fn test_parse_rpa32() {
        let header = b"RPA-3.2 00000000000012ab deadbeef 12345678\n";
        let mut cursor = Cursor::new(header);
        let h = parse_header(&mut cursor).unwrap();
        assert_eq!(h.version, RpaVersion::V3_2);
        assert_eq!(h.index_offset, 0x12ab);
        assert_eq!(h.key, 0xdeadbeef ^ 0x12345678);
    }

    #[test]
    fn test_write_header_roundtrip() {
        let original = "RPA-3.0 00000000000012ab 42424242\n";
        let mut cursor = Cursor::new(original.as_bytes());
        let h = parse_header(&mut cursor).unwrap();
        let written = write_header(h.version, h.index_offset, h.key);
        assert_eq!(written, original);
    }
}
