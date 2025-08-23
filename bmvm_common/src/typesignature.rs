use core::num::NonZeroUsize;

pub trait TypeSignature: Send + Sync {
    const SIGNATURE: u64;
    const IS_PRIMITIVE: bool;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String;
}

macro_rules! impl_type_hash_for_primitive {
    ($($prim:ty),* $(,)?) => {
        $(
            impl TypeSignature for $prim {
                const SIGNATURE: u64 = {
                    let mut h = crate::hash::SignatureHasher::new();
                    h.write(0u64.to_le_bytes().as_slice());
                    h.write(stringify!($prim).as_bytes());
                    h.finish()
                };
                const IS_PRIMITIVE: bool = true;
                #[cfg(feature = "vmi-consume")]
                fn name() -> String {
                    String::from(stringify!($prim))
                }
            }
        )*
    };
}

impl TypeSignature for NonZeroUsize {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(usize::SIGNATURE.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = true;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from("NonZeroUsize")
    }
}

impl_type_hash_for_primitive!(
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    f32,
    f64,
    bool,
    usize,
    (),
);
