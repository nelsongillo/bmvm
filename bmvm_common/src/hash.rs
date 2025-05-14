/// variation on the DJB2 hash algorithm limited to 32bit integer
pub struct Djb2 {
    hash: u32,
}

impl Djb2 {
    pub fn new() -> Self {
        // 5381 is the initial hash value for DJB2
        Self { hash: 5381 }
    }

    pub fn write(&mut self, input: &[u8]) {
        for byte in input.iter() {
            self.hash = self
                .hash
                .wrapping_shl(5)
                .wrapping_add(self.hash)
                .wrapping_add(byte.clone() as u32);
        }
    }

    pub fn finish(self) -> u32 {
        self.hash
    }

    pub fn hash(input: &[u8]) -> u32 {
        let mut hasher = Self::new();
        hasher.write(input);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_djb2() {
        let zero = Djb2::new();
        assert_eq!(zero.finish(), 5381);

        let mut hello_en = Djb2::new();
        hello_en.write("hello".as_bytes());
        assert_eq!(hello_en.finish(), 261238937);

        let mut hello_de = Djb2::new();
        hello_de.write("hallo".as_bytes());
        assert_eq!(hello_de.finish(), 261095189);
    }
}
