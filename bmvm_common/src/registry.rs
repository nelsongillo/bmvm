use crate::TypeSignature;
use crate::hash::Djb2;

/// This trait is used to enforce the rule that functions intended for cross-boundary calls must
/// have parameters which are either primitives implementing the `Type` trait or passable messages.
/// To be able to be a passable message, the type must
/// * Sized
/// * be `repr(C)` or `repr(transparent)` (where the single field must implement `Msg`)
#[sealed::sealed]
pub trait Params: Sized {}

// explicitly impl Params for (), as it would be more trouble to have a special case for it,
// due to the special TypeSignature implementation
#[sealed::sealed]
impl Params for () {}

macro_rules! for_each_function_signature {
    ($mac:ident) => {
        $mac!(T1);
        $mac!(T1 T2);
        $mac!(T1 T2 T3);
        $mac!(T1 T2 T3 T4);
        $mac!(T1 T2 T3 T4 T5);
        $mac!(T1 T2 T3 T4 T5 T6);
        $mac!(T1 T2 T3 T4 T5 T6 T7);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16);
        $mac!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17);
    };
}

macro_rules! impl_params_and_typesignature {
    ($($t:ident)*) => (
        #[allow(unused_parens)]
        #[sealed::sealed]
        impl<$($t),*> Params for ($($t,)*)  where
            $($t: TypeSignature,)* {}

        #[allow(unused_variables)]
        #[allow(unused_assignments)]
        impl<$($t),*> TypeSignature for ($($t,)*)  where
            $($t: TypeSignature,)*
        {
            const IS_PRIMITIVE: bool = false;
            const SIGNATURE: u64 = {
                let mut index: u64 = 0;
                let mut hasher = Djb2::new();
                $(
                    hasher.write(&index.to_le_bytes());
                    hasher.write(&<$t as TypeSignature>::SIGNATURE.to_le_bytes());
                    index += 1;
                )*
                hasher.finish()
            };
        }
    );
}

for_each_function_signature!(impl_params_and_typesignature);
