pub trait Djb2 {
    type Output;

    const OFFSET: Self::Output;

    fn write(&mut self, input: &[u8]);
    fn finish(self) -> Self::Output;
    fn hash(input: &[u8]) -> Self::Output;
}

/// variation on the DJB2 hash algorithm limited to 32bit integer
pub struct Djb232(u32);

impl Djb232 {

    pub fn new() -> Self {
        // 5381 is the initial hash value for DJB2
        Self(Self::OFFSET)
    }
}

impl Djb2 for Djb232 {
    type Output = u32;
    const OFFSET: Self::Output = 5381;

    fn write(&mut self, input: &[u8]) {
        for byte in input.iter() {
            self.0 = self
                .0
                .wrapping_shl(5)
                .wrapping_add(self.0)
                .wrapping_add(byte.clone() as  Self::Output);
        }
    }

    fn finish(self) ->  Self::Output {
        self.0
    }

    fn hash(input: &[u8]) ->  Self::Output {
        let mut hasher = Djb232::new();
        hasher.write(input);
        hasher.finish()
    }
}


/// variation on the DJB2 hash algorithm limited to 32bit integer
pub struct Djb264(u64);

impl Djb264 {

    pub fn new() -> Self {
        Self(Self::OFFSET)
    }
}

impl Djb2 for Djb264 {
    type Output = u64;
    const OFFSET: Self::Output = 5381;


    fn write(&mut self, input: &[u8]) {
        for byte in input.iter() {
            self.0 = self
                .0
                .wrapping_shl(5)
                .wrapping_add(self.0)
                .wrapping_add(byte.clone() as Self::Output);
        }
    }

    fn finish(self) -> Self::Output {
        self.0
    }

    fn hash(input: &[u8]) ->  Self::Output {
        let mut hasher = Djb264::new();
        hasher.write(input);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_djb2() {
        let zero = Djb232::new();
        assert_eq!(zero.finish(), 5381);

        let mut hello_en = Djb232::new();
        hello_en.write("hello".as_bytes());
        assert_eq!(hello_en.finish(), 261238937);

        let mut hello_de = Djb232::new();
        hello_de.write("hallo".as_bytes());
        assert_eq!(hello_de.finish(), 261095189);
    }
}
