//! Stable 64-bit FNV-1a hash. Never use `DefaultHasher` (its output is not stable).

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hash a byte slice with FNV-1a (64 bit).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = Fnv1a::new();
    h.write(bytes);
    h.finish()
}

/// Incremental FNV-1a hasher.
#[derive(Clone, Copy, Debug)]
pub struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv1a {
    /// Start a new hash.
    pub fn new() -> Self {
        Self(OFFSET)
    }

    /// Add bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    /// Add a `u32` in little-endian order.
    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    /// Return the hash value.
    pub fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_known_answers() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn incremental_equals_one_shot() {
        let mut h = Fnv1a::new();
        h.write(b"foo");
        h.write(b"bar");
        assert_eq!(h.finish(), fnv1a(b"foobar"));
    }
}
