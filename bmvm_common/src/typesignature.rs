use core::num::NonZeroUsize;

pub trait TypeSignature: Send + Sync {
    const SIGNATURE: u64;
    const IS_PRIMITIVE: bool;
}

macro_rules! impl_type_hash_for_primitive {
    ($($prim:ty => $str:expr),* $(,)?) => {
        $(
            impl TypeSignature for $prim {
                const SIGNATURE: u64 = {
                    let mut h = crate::hash::Djb2::new();
                    h.write(0u64.to_le_bytes().as_slice());
                    h.write($str.as_bytes());
                    h.finish()
                };
                const IS_PRIMITIVE: bool = true;
            }
        )*
    };
}

impl TypeSignature for NonZeroUsize {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(usize::SIGNATURE.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = true;
}

impl_type_hash_for_primitive!(
    u8 => "u8",
    u16 => "u16",
    u32 => "u32",
    u64 => "u64",
    u128 => "u128",
    i8 => "i8",
    i16 => "i16",
    i32 => "i32",
    i64 => "i64",
    i128 => "i128",
    f32 => "f32",
    f64 => "f64",
    bool => "bool",
    char => "char",
    usize => "usize",
    () => "()",
);
