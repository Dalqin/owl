//! Reads the header of a TES3-format plugin (`.esp`, `.esm`, `.omwaddon`,
//! `.omwgame`) to find which master files it depends on.
//!
//! Layout: a `TES3` record (type, u32 size, u32 unused, u32 flags) whose data
//! is a sequence of subrecords (4-byte name, u32 size, data). Each `MAST`
//! subrecord holds a master file name as a NUL-terminated string.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

const RECORD_HEADER_LEN: usize = 16;
/// The TES3 header is a few hundred bytes plus ~40 per master; anything past
/// this is not a real plugin header.
const MAX_HEADER_LEN: usize = 1 << 20;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHeader {
    /// Master file names in the order the plugin lists them.
    pub masters: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("not a TES3 plugin (missing TES3 header record)")]
    NotTes3,
    #[error("plugin header is truncated or malformed")]
    Malformed,
    #[error("plugin header is implausibly large ({0} bytes)")]
    TooLarge(usize),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Parses a header from bytes beginning at the start of a plugin file. Only
/// the first record is read; trailing bytes are ignored.
pub fn parse_header(bytes: &[u8]) -> Result<PluginHeader, PluginError> {
    if bytes.len() < RECORD_HEADER_LEN || &bytes[..4] != b"TES3" {
        return Err(PluginError::NotTes3);
    }
    let size = read_u32(bytes, 4)? as usize;
    let end = RECORD_HEADER_LEN
        .checked_add(size)
        .ok_or(PluginError::Malformed)?;
    let data = bytes
        .get(RECORD_HEADER_LEN..end)
        .ok_or(PluginError::Malformed)?;

    let mut masters = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let name = data.get(pos..pos + 4).ok_or(PluginError::Malformed)?;
        let len = read_u32(data, pos + 4)? as usize;
        let body_start = pos + 8;
        let body = data
            .get(body_start..body_start.checked_add(len).ok_or(PluginError::Malformed)?)
            .ok_or(PluginError::Malformed)?;
        if name == b"MAST" {
            let text = body.split(|&b| b == 0).next().unwrap_or(&[]);
            // Morrowind uses Windows-1252; file names are almost always ASCII.
            masters.push(String::from_utf8_lossy(text).into_owned());
        }
        pos = body_start + len;
    }
    Ok(PluginHeader { masters })
}

/// Reads only as much of the file as the header needs.
pub fn read_header(path: &Path) -> Result<PluginHeader, PluginError> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; RECORD_HEADER_LEN];
    file.read_exact(&mut buf).map_err(|e| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => PluginError::NotTes3,
        _ => PluginError::Io(e),
    })?;
    if &buf[..4] != b"TES3" {
        return Err(PluginError::NotTes3);
    }
    let size = read_u32(&buf, 4)? as usize;
    if size > MAX_HEADER_LEN {
        return Err(PluginError::TooLarge(size));
    }
    buf.resize(RECORD_HEADER_LEN + size, 0);
    file.read_exact(&mut buf[RECORD_HEADER_LEN..])
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::UnexpectedEof => PluginError::Malformed,
            _ => PluginError::Io(e),
        })?;
    parse_header(&buf)
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, PluginError> {
    let b = bytes.get(at..at + 4).ok_or(PluginError::Malformed)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
pub(crate) mod testutil {
    /// Builds a minimal valid TES3 header with the given masters.
    pub fn build_plugin(masters: &[&str]) -> Vec<u8> {
        let mut data = Vec::new();
        // HEDR: f32 version, u32 type, 32-byte company, 256-byte description, u32 record count
        let mut hedr = Vec::new();
        hedr.extend_from_slice(&1.3f32.to_le_bytes());
        hedr.extend_from_slice(&0u32.to_le_bytes());
        hedr.extend_from_slice(&[0u8; 32]);
        hedr.extend_from_slice(&[0u8; 256]);
        hedr.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"HEDR");
        data.extend_from_slice(&(hedr.len() as u32).to_le_bytes());
        data.extend_from_slice(&hedr);
        for m in masters {
            let mut name = m.as_bytes().to_vec();
            name.push(0);
            data.extend_from_slice(b"MAST");
            data.extend_from_slice(&(name.len() as u32).to_le_bytes());
            data.extend_from_slice(&name);
            data.extend_from_slice(b"DATA");
            data.extend_from_slice(&8u32.to_le_bytes());
            data.extend_from_slice(&0u64.to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"TES3");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::build_plugin;
    use super::*;

    #[test]
    fn parses_masters_in_order() {
        let bytes = build_plugin(&["Morrowind.esm", "Tribunal.esm", "Bloodmoon.esm"]);
        let h = parse_header(&bytes).unwrap();
        assert_eq!(
            h.masters,
            vec!["Morrowind.esm", "Tribunal.esm", "Bloodmoon.esm"]
        );
    }

    #[test]
    fn plugin_with_no_masters() {
        assert!(parse_header(&build_plugin(&[])).unwrap().masters.is_empty());
    }

    #[test]
    fn rejects_non_tes3() {
        assert!(matches!(
            parse_header(b"placeholder\n padding..."),
            Err(PluginError::NotTes3)
        ));
        assert!(matches!(parse_header(b""), Err(PluginError::NotTes3)));
    }

    #[test]
    fn rejects_truncated() {
        let mut bytes = build_plugin(&["Morrowind.esm"]);
        bytes.truncate(bytes.len() - 20);
        assert!(matches!(parse_header(&bytes), Err(PluginError::Malformed)));
    }

    #[test]
    fn ignores_trailing_records() {
        let mut bytes = build_plugin(&["Morrowind.esm"]);
        bytes.extend_from_slice(b"GMST\x04\0\0\0\0\0\0\0\0\0\0\0\xff\xff\xff\xff");
        assert_eq!(parse_header(&bytes).unwrap().masters, vec!["Morrowind.esm"]);
    }
}
