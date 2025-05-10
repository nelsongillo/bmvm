/// variation on teh DJB2 hash algorithm limited to 32bit integer
pub fn djb2_u32(input: &str) -> u32 {
    let mut hash: u32 = 5381;

    for byte in input.bytes() {
        // hash = (hash * 33) + byte
        hash = hash
            .wrapping_shl(5)
            .wrapping_add(hash)
            .wrapping_add(byte as u32);
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_djb2() {
        assert_eq!(djb2_u32(""), 5381);
        assert_eq!(djb2_u32("hello"), 261238937);
        assert_eq!(djb2_u32("hello!"), 30950362);
    }
}
