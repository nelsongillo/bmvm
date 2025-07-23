use core::convert::TryFrom;
use core::result::Result;
use sealed::sealed;
use crate::vmi::Signature;

pub const META_PREFIX_HOST: &str = "BMVM_CALL_HOST_META_";
pub const META_PREFIX_GUEST: &str = "BMVM_CALL_GUEST_META_";
pub const LINK_META_NAME_HOST: &str = ".bmvm.call.host";
pub const LINK_META_NAME_GUEST: &str = ".bmvm.call.guest";

/// Functions intended for cross-boundary calls are only allowed to have a have parameters which implement the `Type` trait.
/// This rule is enforced by the compiler via the `Params` trait.
/// Currently, only 8, 16, 32 and 64 integer types are supported, in addition to `*const u8` pointer.
#[sealed]
pub trait Type {}

macro_rules! impl_type {
    ($($s:tt = $t:ty),*) => {
        $(
            #[sealed]
            impl Type for $t {}
        )*

        #[repr(u8)]
        #[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Hash)]
        pub enum DataType {
            $(
                $s,
            )*
        }

        impl From<DataType> for u8 {
            fn from(data_type: DataType) -> u8 {
                data_type as u8
            }
        }

        impl TryFrom<u8> for DataType {
            type Error = &'static str;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $(
                        i if i == DataType::$s as u8 => Ok(DataType::$s),
                    )*
                    _ => Err("Invalid DataType value"),
                }
            }
        }

    };
}

impl_type!(
    UInt8 = u8,
    UInt16 = u16,
    UInt32 = u32,
    UInt64 = u64,
    Int8 = i8,
    Int16 = i16,
    Int32 = i32,
    Int64 = i64
);

/// This trait is used to enforce the rule that functions intended for cross-boundary calls must
/// have parameters which are either primitives implementing the `Type` trait or passable messages.
/// To be able to be a passable message, the type must
/// * Sized
/// * be `repr(C)` or `repr(transparent)` (where the single field must implement `Msg`)
pub trait Params: Sized {}

macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!(0);
        $mac!(1 T1);
        $mac!(2 T1 T2);
        $mac!(3 T1 T2 T3);
        $mac!(4 T1 T2 T3 T4);
        $mac!(5 T1 T2 T3 T4 T5);
        $mac!(6 T1 T2 T3 T4 T5 T6);
        $mac!(7 T1 T2 T3 T4 T5 T6 T7);
        $mac!(8 T1 T2 T3 T4 T5 T6 T7 T8);
        $mac!(9 T1 T2 T3 T4 T5 T6 T7 T8 T9);
        $mac!(10 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
        $mac!(11 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
        $mac!(12 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
        $mac!(13 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13);
        $mac!(14 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14);
        $mac!(15 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
        $mac!(16 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16);
        $mac!(17 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17);
    };
}

macro_rules! impl_params {
    ($n:tt $($t:ident)*) => (
        #[allow(unused_parens)]
        impl<$($t),*> Params for ($($t),*)  where
            $($t: Type,)* {}
    );
}

for_each_function_signature!(impl_params);

pub type FuncId = u32;

pub type FnCrossCall = fn(&[u8]) -> &[u8];

pub enum CallDirection {
    HostToGuest,
    GuestToHost,
}

pub struct CrossFunction {
    sig: Signature,
    call_direction: CallDirection,
    func: FnCrossCall,
}

// inventory::collect!(CrossFunction);
