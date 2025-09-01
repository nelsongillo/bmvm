/// Signature hasher implements the hashing algorithm used to compute function and type
/// signatures. The here-used algorithm is DJB2.
#[repr(transparent)]
pub struct SignatureHasher(u64);

impl SignatureHasher {
    const OFFSET: u64 = 5381;

    /// Create a new hasher instance
    pub const fn new() -> Self {
        Self(Self::OFFSET)
    }

    /// Create a new hasher instance from a partial result. Useful for incremental hashing.
    pub const fn from_partial(partial: u64) -> Self {
        Self(partial)
    }

    /// Write input to the hasher.
    pub const fn write(&mut self, input: &[u8]) {
        let mut i = 0;
        while i < input.len() {
            self.0 = self
                .0
                .wrapping_shl(5)
                .wrapping_add(self.0)
                .wrapping_add(input[i] as u64);
            i += 1;
        }
    }

    /// Get the final hash value.
    pub const fn finish(self) -> u64 {
        self.0
    }

    /// Hash an input. Same as creating a new instance, writing the single input and finishing.
    ///
    /// ```rust
    /// use bmvm_common::hash::SignatureHasher;
    ///
    /// let input = b"hello, World!";
    ///
    /// let mut verbose = SignatureHasher::new();
    /// verbose.write(input);
    ///
    /// assert_eq!(verbose.finish(), SignatureHasher::hash(input));
    /// ```
    pub const fn hash(input: &[u8]) -> u64 {
        let mut hasher = SignatureHasher::new();
        hasher.write(input);
        hasher.finish()
    }
}

impl Default for SignatureHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let zero = SignatureHasher::new();
        assert_eq!(5381, zero.finish());

        let mut hello_en = SignatureHasher::new();
        hello_en.write("hello".as_bytes());
        assert_eq!(210714636441, hello_en.finish());

        let mut hello_de = SignatureHasher::new();
        hello_de.write("hallo".as_bytes());
        assert_eq!(210714492693, hello_de.finish());
    }

    #[test]
    fn differentiate_based_on_name() {
        let mut a = SignatureHasher::new();
        a.write("hello".as_bytes());
        a.write(0u64.to_le_bytes().as_slice());
        a.write(b"u64");

        let mut b = SignatureHasher::new();
        b.write("world".as_bytes());
        b.write(0u64.to_le_bytes().as_slice());
        b.write(b"u64");

        assert_ne!(a.finish(), b.finish());
    }
}
