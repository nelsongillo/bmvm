use crate::TypeSignature;
use crate::error::ExitCode;
use crate::mem::{
    Error as MemError, Foreign, ForeignBuf, OffsetPtr, RawOffsetPtr, Shared, SharedBuf, get_foreign,
};
use core::num::NonZeroUsize;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct Transport {
    /// primary can be a u32 offset pointer or a primitive integer/float/bool value
    primary: u64,
    /// Secondary is optional, it is only used as the capacity if a buffer is shared.
    /// If unused, it should be 0
    secondary: u64,
}

impl Transport {
    pub fn new(primary: u64, secondary: u64) -> Self {
        Self { primary, secondary }
    }

    pub fn primary(&self) -> u64 {
        self.primary
    }

    pub fn secondary(&self) -> u64 {
        self.secondary
    }
}

#[cfg(feature = "vmi-consume")]
impl core::fmt::Display for Transport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[sealed::sealed(pub(crate))]
pub trait OwnedShareable: TypeSignature {
    fn into_transport(self) -> Transport;
}

#[sealed::sealed(pub(crate))]
pub trait ForeignShareable: TypeSignature {
    fn from_transport(t: Transport) -> Result<Self, ExitCode>
    where
        Self: Sized;
}

#[sealed::sealed]
impl ForeignShareable for ForeignBuf {
    fn from_transport(t: Transport) -> Result<Self, ExitCode> {
        if t.secondary == 0 {
            return Err(ExitCode::ZeroCapacity);
        }

        let raw_capacity = t.secondary as usize;
        let capacity = NonZeroUsize::new(raw_capacity).ok_or(ExitCode::ZeroCapacity)?;

        let raw = RawOffsetPtr::from(t.primary as u32);
        let ptr = OffsetPtr::from(raw);

        Ok(ForeignBuf { ptr, capacity })
    }
}

#[sealed::sealed]
impl<T: TypeSignature> ForeignShareable for Foreign<T> {
    fn from_transport(t: Transport) -> Result<Self, ExitCode> {
        let raw = RawOffsetPtr::from(t.primary as u32);
        let ptr = OffsetPtr::from(raw);
        unsafe {
            get_foreign(ptr).map_err(|e| match e {
                MemError::UninitializedAllocator => ExitCode::NullPtr,
                MemError::NullPointer => ExitCode::NullPtr,
                _ => ExitCode::Ptr(raw),
            })
        }
    }
}

#[sealed::sealed]
impl<T: TypeSignature> OwnedShareable for Shared<T> {
    fn into_transport(self) -> Transport {
        Transport {
            primary: self.inner.offset as u64,
            secondary: 0,
        }
    }
}

#[sealed::sealed]
impl OwnedShareable for SharedBuf {
    fn into_transport(self) -> Transport {
        Transport {
            primary: self.ptr.offset as u64,
            secondary: self.capacity.get() as u64,
        }
    }
}

macro_rules! impl_owned_shareable_for_primitives {
    ($($prim:ty),* $(,)?) => {
        $(
            #[sealed::sealed]
            impl OwnedShareable for $prim {
                #[inline(always)]
                fn into_transport(self) -> Transport {
                    Transport {
                        primary: self as u64,
                        secondary: 0,
                    }
                }
            }
        )*
    };
}

macro_rules! impl_foreign_shareable_for_primitives {
    ($($prim:ty),* $(,)?) => {
        $(
            #[sealed::sealed]
            impl ForeignShareable for $prim {
                 fn from_transport(t: Transport) -> Result<Self, ExitCode> {
                   Ok(t.primary as $prim)
                 }
            }
        )*
    };
}

impl_owned_shareable_for_primitives!(
    u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, usize, bool
);
impl_foreign_shareable_for_primitives!(
    u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, usize
);

#[sealed::sealed]
impl OwnedShareable for () {
    fn into_transport(self) -> Transport {
        Transport {
            primary: 0,
            secondary: 0,
        }
    }
}

#[sealed::sealed]
impl ForeignShareable for () {
    fn from_transport(_: Transport) -> Result<Self, ExitCode> {
        Ok(())
    }
}

#[sealed::sealed]
impl ForeignShareable for bool {
    fn from_transport(t: Transport) -> Result<Self, ExitCode> {
        Ok(t.primary != 0)
    }
}
