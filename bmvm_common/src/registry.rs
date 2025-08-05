use crate::TypeSignature;

/// This trait is used to enforce the rule that functions intended for cross-boundary calls must
/// have parameters which are either primitives implementing the `Type` trait or passable messages.
/// To be able to be a passable message, the type must
/// * Sized
/// * be `repr(C)` or `repr(transparent)` (where the single field must implement `Msg`)
#[sealed::sealed]
pub trait Params: TypeSignature {}

// explicitly impl Params for (), as it would be more trouble to have a special case for it,
// due to the special TypeSignature implementation
#[sealed::sealed]
impl Params for () {}

#[sealed::sealed]
impl<T: TypeSignature> Params for (T,) {}

impl<T: TypeSignature> TypeSignature for (T,) {
    // inherit signature from, as single value tuples should be treated as non-tuple values
    const SIGNATURE: u64 = T::SIGNATURE;
    // inherit primitive state from, as single value tuples should be treated as non-tuple values
    const IS_PRIMITIVE: bool = T::IS_PRIMITIVE;
}

macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!("2" T1 T2);
        $mac!("3" T1 T2 T3);
        $mac!("4" T1 T2 T3 T4);
        $mac!("5" T1 T2 T3 T4 T5);
        $mac!("6" T1 T2 T3 T4 T5 T6);
        $mac!("7" T1 T2 T3 T4 T5 T6 T7);
        $mac!("8" T1 T2 T3 T4 T5 T6 T7 T8);
        $mac!("9" T1 T2 T3 T4 T5 T6 T7 T8 T9);
        $mac!("10" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
        $mac!("11" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
        $mac!("12" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
        $mac!("13" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13);
        $mac!("14" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14);
        $mac!("15" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
        $mac!("16" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16);
        $mac!("17" T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17);
    };
}

macro_rules! impl_params_and_typesignature {
    ($n:literal $($t:ident)*) => (
        #[repr(C)]
        pub struct ${concat(Tuple, $n)} <$($t),*>
        where
            $($t: TypeSignature,)*
        {
            $(pub $t: $t),*
        }

        impl<$($t),+> From<($($t,)+)> for ${concat(Tuple, $n)} <$($t),*>
        where
            $($t: TypeSignature,)*
        {
            fn from(tuple: ($($t,)+)) -> Self {
                let ($($t,)+) = tuple;
                Self { $($t),+ }
            }
        }

        #[allow(unused_variables)]
        #[allow(unused_assignments)]
        impl<$($t),*> TypeSignature for ${concat(Tuple, $n)}  <$($t),*>
        where
            $($t: TypeSignature,)*
        {
            const IS_PRIMITIVE: bool = false;
            const SIGNATURE: u64 = {
                let mut index: u64 = 0;
                let mut hasher = SignatureHasher::new();
                $(
                    hasher.write(index.to_le_bytes().as_slice());
                    hasher.write(<$t as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
                    index += 1;
                )*
                hasher.finish()
            };
        }

        #[allow(unused_parens)]
        #[sealed::sealed]
        impl<$($t),*> Params for ($($t,)*) where $($t: TypeSignature,)* {}

        #[allow(unused_variables)]
        #[allow(unused_assignments)]
        impl<$($t),*> TypeSignature for ($($t,)*)
        where
            $($t: TypeSignature,)*
        {
            const IS_PRIMITIVE: bool = false;
            const SIGNATURE: u64 = {
                let mut index: u64 = 0;
                let mut hasher = SignatureHasher::new();
                $(
                    hasher.write(index.to_le_bytes().as_slice());
                    hasher.write(<$t as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
                    index += 1;
                )*
                hasher.finish()
            };
        }
    );
}

for_each_function_signature!(impl_params_and_typesignature);
