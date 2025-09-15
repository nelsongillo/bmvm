use core::marker::PhantomData;
use core::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

/// Align address downwards.
///
/// Returns the greatest `x` with alignment `align` so that `x <= addr`.
///
/// Panics if the alignment is not a power of two.
#[inline]
const fn align_down(addr: u64, align: u64) -> u64 {
    assert!(align.is_power_of_two(), "`align` must be a power of two");
    let mask = align - 1;
    if addr & mask == 0 {
        return addr;
    }
    addr & !(align - 1)
}

/// Align address upwards.
///
/// Returns the smallest `x` with alignment `align` so that `x >= addr`.
///
/// Panics if the alignment is not a power of two or if an overflow occurs.
#[inline]
const fn align_up(addr: u64, align: u64) -> u64 {
    assert!(align.is_power_of_two(), "`align` must be a power of two");
    let mask = align - 1;
    if addr & mask == 0 {
        return addr;
    }

    (addr | mask)
        .checked_add(1)
        .expect("attempt to add with overflow")
}

/// This is a quick const wrapper for the DefaultAlign::align_floor function
pub const fn align_floor(addr: u64) -> u64 {
    align_down(addr, DefaultAlign::ALIGNMENT)
}

/// This is a quick const wrapper for the DefaultAlign::align_ceil function
pub const fn align_ceil(addr: u64) -> u64 {
    align_up(addr, DefaultAlign::ALIGNMENT)
}

/// Trait to abstract over different page sizes based on the underlying architecture.
pub trait Align: Copy + Eq + PartialEq + PartialOrd + Ord {
    const ALIGNMENT: u64;

    fn is_aligned(addr: u64) -> bool {
        addr.is_multiple_of(Self::ALIGNMENT)
    }

    /// align an address to the beginning of the page
    fn align_floor(addr: u64) -> u64 {
        align_down(addr, Self::ALIGNMENT)
    }

    /// align an address to the beginning of the next page
    fn align_ceil(addr: u64) -> u64 {
        align_up(addr, Self::ALIGNMENT)
    }
}

#[cfg(target_arch = "x86_64")]
pub type DefaultAlign = X86_64;

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct X86_64;

impl Align for X86_64 {
    const ALIGNMENT: u64 = 0x1000;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Arm64;

impl Align for Arm64 {
    const ALIGNMENT: u64 = 0x1000;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Stack16B;

impl Align for Stack16B {
    const ALIGNMENT: u64 = 0x10;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page4KiB;

impl Align for Page4KiB {
    const ALIGNMENT: u64 = 0x1000;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page2MiB;

impl Align for Page2MiB {
    const ALIGNMENT: u64 = Page4KiB::ALIGNMENT * 512;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Page1GiB;
impl Align for Page1GiB {
    const ALIGNMENT: u64 = Page2MiB::ALIGNMENT * 512;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Stack;
impl Align for Stack {
    const ALIGNMENT: u64 = 16;
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StackPreCall;
impl Align for StackPreCall {
    const ALIGNMENT: u64 = 8;
}

#[sealed::sealed]
pub trait ZeroableInteger {}

macro_rules! impl_zeroable_primitive {
    ($($int:ty),* $(,)?) => {
        $(
        #[sealed::sealed]
        impl ZeroableInteger for $int {}
        )*
    };
}

impl_zeroable_primitive! {
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
}

#[sealed::sealed]
pub trait NonZeroableInteger {}

macro_rules! impl_non_zeroable_primitive {
    ($($int:ty),* $(,)?) => {
        $(
        #[sealed::sealed]
        impl NonZeroableInteger for core::num::NonZero<$int> {}
        )*
    };
}

impl_non_zeroable_primitive! {
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Aligned<I: ZeroableInteger, A: Align = DefaultAlign> {
    inner: I,
    _alignment: PhantomData<A>,
}

/// Aligned non-zero integer wrapper
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct AlignedNonZero<I: NonZeroableInteger, A: Align = DefaultAlign> {
    inner: I,
    _alignment: PhantomData<A>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct ErrZeroValue {}

#[cfg(feature = "vmi-consume")]
impl core::fmt::Display for ErrZeroValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "provided value is zero")
    }
}

macro_rules! impl_aligned {
    ($($int:ty => $aligned_name:ident),* $(,)?) => {
        $(
        #[allow(type_alias_bounds)]
        /// Aligned non-zero type for $int
        pub type $aligned_name<A: Align = DefaultAlign> = Aligned<$int, A>;

        impl<A: Align> Aligned<$int, A> {
            pub const fn zero() -> Self {
                Self {
                    inner: 0,
                    _alignment: PhantomData,
                }
            }

            /// Creates a new aligned value with floor alignment
            pub fn new_floor(value: $int) -> Self {
                Self {
                    inner: A::align_floor(value as u64) as $int,
                    _alignment: PhantomData,
                }
            }

            /// Creates a new aligned value with ceiling alignment
            pub fn new_ceil(value: $int) -> Self {
                Self {
                    inner: A::align_ceil(value as u64) as $int,
                    _alignment: PhantomData,
                }
            }

            /// Creates from already aligned non-zero value
            pub fn new_aligned(value: $int) -> Option<Self> {
                if A::is_aligned(value as u64) {
                    Some(Self {
                        inner: value,
                        _alignment: PhantomData,
                    })
                } else {
                    None
                }
            }

            #[inline]
            pub const fn new_unchecked(value: $int) -> Self {
                Self {
                    inner: value,
                    _alignment: PhantomData,
                }
            }

            /// Returns the inner value
            #[inline]
            pub const fn get(&self) -> $int {
                self.inner
            }
        }

        impl<A: Align> From<Aligned<$int, A>> for $int {
            fn from(value: Aligned<$int, A>) -> $int {
                value.get()
            }
        }
        )*
    };
}

impl_aligned! {
    u32 => AlignedU32,
    u64 => AlignedU64,
    usize => AlignedUsize,
}

/// Macro to implementation for selected NonZero integer types and creation of type aliases
macro_rules! impl_aligned_non_zero {
    ($($int:ty => $nonzero:ty => $aligned_name:ident),* $(,)?) => {
        $(
        #[allow(type_alias_bounds)]
        /// Aligned non-zero type for $int
        pub type $aligned_name<A: Align = DefaultAlign> = AlignedNonZero<$nonzero, A>;

        impl<A: Align> AlignedNonZero<$nonzero, A> {
            /// Creates a new aligned value with floor alignment
            pub fn new_floor(value: $int) -> Option<Self> {
                let aligned = A::align_floor(value as u64) as $int;
                <$nonzero>::new(aligned as $int).map(|inner| Self {
                    inner,
                    _alignment: PhantomData,
                })
            }

            /// Creates a new aligned value with ceiling alignment
            pub fn new_ceil(value: $int) -> Option<Self> {
                let aligned = A::align_ceil(value as u64) as $int;
                <$nonzero>::new(aligned as $int).map(|inner| Self {
                    inner,
                    _alignment: PhantomData,
                })
            }

            /// Creates from already aligned non-zero value
            pub fn new_aligned(value: $int) -> Option<Self> {
                if A::is_aligned(value as u64) {
                    <$nonzero>::new(value).map(|inner| Self {
                        inner,
                        _alignment: PhantomData,
                    })
                } else {
                    None
                }
            }

            /// Creates from already aligned non-zero value without checking alignment but still
            /// zero checking
            pub fn from_aligned(value: $int) -> Option<Self> {
                <$nonzero>::new(value).map(|inner| Self {
                    inner,
                    _alignment: PhantomData,
                })
            }

            #[inline]
            pub const fn new_unchecked(value: $nonzero) -> Self {
                Self {
                    inner: value,
                    _alignment: PhantomData,
                }
            }

            /// Create from a raw value without any alignment checks
            ///
            /// # Safety
            /// Passing a zero value or an unaligned value can cause undefined behaviour in
            /// applications relying on those properties.
            #[inline]
            pub const unsafe fn new_unchecked_raw(value: $int) -> Self {
                Self {
                    inner: unsafe { <$nonzero>::new_unchecked(value) },
                    _alignment: PhantomData,
                }
            }

            /// Returns the inner value
            #[inline]
            pub const fn get(&self) -> $int {
                self.inner.get()
            }

            /// Returns the raw NonZero value
            #[inline]
            pub const fn get_non_zero(self) -> $nonzero {
                self.inner
            }
        }

        impl<A: Align> From<AlignedNonZero<$nonzero, A>> for $int {
            fn from(value: AlignedNonZero<$nonzero, A>) -> $int {
                value.get()
            }
        }
        )*
    };
}

impl_aligned_non_zero! {
    u32 => NonZeroU32 => AlignedNonZeroU32,
    u64 => NonZeroU64 => AlignedNonZeroU64,
    usize => NonZeroUsize => AlignedNonZeroUsize,
}

macro_rules! impl_aligned_to_non_zero_aligned {
    ($($zeroable:ident : $nonzero:ident => $aligned:ident),* $(,)?) => {
        $(
        impl<A: Align> TryFrom<$zeroable<A>> for $aligned<A> {
            type Error = ErrZeroValue;

            fn try_from(value: $zeroable<A>) -> core::result::Result<Self, Self::Error> {
                $aligned::from_aligned(value.get()).ok_or(ErrZeroValue{})
            }
        }

        impl<A: Align> From<$aligned<A>> for $zeroable<A> {
            fn from(value: $aligned<A>) -> $zeroable<A> {
                Self {
                    inner: value.get(),
                    _alignment: PhantomData,
                }
            }
        }
        )*
    };
}

impl_aligned_to_non_zero_aligned! {
    AlignedU32 : NonZeroU32 => AlignedNonZeroU32,
    AlignedU64 : NonZeroU64 => AlignedNonZeroU64,
    AlignedUsize : NonZeroUsize => AlignedNonZeroUsize,
}
