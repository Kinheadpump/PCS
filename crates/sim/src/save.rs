//! Save format (TECHNICAL_DESIGN §9).
//!
//! Layout: magic `COLSAVE1` (8 bytes), `format_version: u32`, `content_hash: u64`,
//! `state_hash: u64` (all little endian), then the `postcard` payload of the `Sim`.
//! This crate only makes and reads bytes. The `game` crate does the file I/O.

use crate::Sim;
use base::hash::fnv1a;
use std::fmt;

/// Magic bytes at the start of a save.
pub const MAGIC: [u8; 8] = *b"COLSAVE1";
/// Version of the payload layout. Before version 1.0 a different version is refused.
pub const FORMAT_VERSION: u32 = 2;
/// Size of the header in bytes.
pub const HEADER_LEN: usize = 8 + 4 + 8 + 8;

/// Why a save could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    /// The data is shorter than the header.
    TooShort,
    /// The data does not start with `COLSAVE1`.
    BadMagic,
    /// The save has another format version. The game cannot read it.
    UnsupportedVersion {
        /// Version in the save.
        found: u32,
        /// Version that this build reads.
        expected: u32,
    },
    /// The payload is damaged or not a valid state.
    Corrupt,
    /// The payload hash differs from the hash in the header.
    HashMismatch,
    /// The content hash in the header differs from the one in the state.
    ContentMismatch,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::TooShort => write!(f, "save file is too short"),
            SaveError::BadMagic => write!(f, "file is not a colony save (wrong magic bytes)"),
            SaveError::UnsupportedVersion { found, expected } => write!(
                f,
                "save has format version {found}; this game reads version {expected}"
            ),
            SaveError::Corrupt => write!(f, "save data is damaged"),
            SaveError::HashMismatch => write!(f, "save data does not match its hash"),
            SaveError::ContentMismatch => write!(f, "save content hash is wrong"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Make the bytes of a save.
pub fn to_bytes(sim: &Sim) -> Vec<u8> {
    let payload = sim.encode_state();
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&sim.content_hash().to_le_bytes());
    out.extend_from_slice(&fnv1a(&payload).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Read a save. Damaged bytes give an `Err`; this function never panics. Derived
/// caches are rebuilt.
pub fn from_bytes(bytes: &[u8]) -> Result<Sim, SaveError> {
    if bytes.len() < HEADER_LEN {
        return Err(SaveError::TooShort);
    }
    if bytes[..8] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = u32::from_le_bytes(array(&bytes[8..12]));
    if version != FORMAT_VERSION {
        return Err(SaveError::UnsupportedVersion {
            found: version,
            expected: FORMAT_VERSION,
        });
    }
    let content_hash = u64::from_le_bytes(array(&bytes[12..20]));
    let state_hash = u64::from_le_bytes(array(&bytes[20..28]));
    let payload = &bytes[HEADER_LEN..];
    if fnv1a(payload) != state_hash {
        return Err(SaveError::HashMismatch);
    }
    let sim = Sim::decode_state(payload).ok_or(SaveError::Corrupt)?;
    if sim.content_hash() != content_hash {
        return Err(SaveError::ContentMismatch);
    }
    Ok(sim)
}

/// Copy a slice of the right length into an array. A wrong length gives zeros; the
/// callers pass fixed-size slices.
fn array<const N: usize>(slice: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    if slice.len() == N {
        out.copy_from_slice(slice);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::sim_from_ascii;

    #[test]
    fn header_layout() {
        let sim = sim_from_ascii("c..", 1).unwrap();
        let bytes = to_bytes(&sim);
        assert_eq!(&bytes[..8], b"COLSAVE1");
        assert_eq!(u32::from_le_bytes(array(&bytes[8..12])), FORMAT_VERSION);
        assert_eq!(
            u64::from_le_bytes(array(&bytes[20..28])),
            sim.state_hash(),
            "header holds the state hash"
        );
    }

    #[test]
    fn rejects_short_magic_and_version() {
        let sim = sim_from_ascii("c..", 1).unwrap();
        let bytes = to_bytes(&sim);
        assert_eq!(from_bytes(&bytes[..10]).unwrap_err(), SaveError::TooShort);
        let mut bad = bytes.clone();
        bad[0] = b'X';
        assert_eq!(from_bytes(&bad).unwrap_err(), SaveError::BadMagic);
        let mut bad = bytes.clone();
        bad[8] = 9;
        assert!(matches!(
            from_bytes(&bad).unwrap_err(),
            SaveError::UnsupportedVersion { found: 9, .. }
        ));
        assert!(from_bytes(&bytes).is_ok());
    }
}
