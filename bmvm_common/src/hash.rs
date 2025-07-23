/// variation on the DJB2 hash algorithm limited to 32bit integer
#[repr(transparent)]
pub struct Djb2(u64);

impl Djb2 {
    const OFFSET: u64 = 5381;

    /// Create a new Djb2 hasher instance
    pub const fn new() -> Self {
        Self(Self::OFFSET)
    }

    /// Create a new Djb2 hasher instance from a partial result. Useful for incremental hashing.
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
    /// use bmvm_common::hash::Djb2;
    ///
    /// let input = b"hello, World!";
    ///
    /// let mut verbose = Djb2::new();
    /// verbose.write(input);
    ///
    /// assert_eq!(verbose.finish(), Djb2::hash(input));
    /// ```
    pub const fn hash(input: &[u8]) -> u64 {
        let mut hasher = Djb2::new();
        hasher.write(input);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let zero = Djb2::new();
        assert_eq!(5381, zero.finish());

        let mut hello_en = Djb2::new();
        hello_en.write("hello".as_bytes());
        assert_eq!(210714636441, hello_en.finish());

        let mut hello_de = Djb2::new();
        hello_de.write("hallo".as_bytes());
        assert_eq!(210714492693, hello_de.finish());
    }
}
