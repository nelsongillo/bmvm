use bmvm_common::error::ExitCode;
use bmvm_common::mem::Transport;
use bmvm_common::vmi::{FnCall, Signature};
use bmvm_common::{TypeSignature, vmi};
use std::cmp::Ordering;
use std::ffi::IntoStringError;

inventory::collect!(CallableFunction);

pub type HypercallResult = Result<Transport, ExitCode>;

pub type WrapperFunc = extern "C" fn(Transport) -> HypercallResult;

pub struct CallableFunction {
    /// serialized FnCall
    pub meta: &'static [u8],
    /// Pointer to the wrapper function
    pub func: WrapperFunc,
}

#[derive(Debug)]
pub struct Function {
    pub sig: Signature,
    pub name: String,
    pub params: Vec<String>,
    pub output: Option<String>,
    pub func: WrapperFunc,
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("Invalid string: {0}")]
    InvalidString(#[from] IntoStringError),
    #[error("Error parsing function metadata: {0}")]
    ParseError(#[from] vmi::Error),
}

impl TryFrom<&CallableFunction> for Function {
    type Error = ConversionError;

    fn try_from(value: &CallableFunction) -> Result<Self, Self::Error> {
        let call = FnCall::try_from_bytes(value.meta, true)?;
        let name = call.name.into_string()?;
        let sig = call.sig;
        let func = value.func;
        let params: Vec<String> = call
            .debug_param_types
            .iter()
            .map(|p| p.to_owned().into_string())
            .try_collect::<Vec<String>>()?;
        let output = match call.debug_return_type {
            Some(o) => Some(o.into_string()?),
            None => None,
        };

        Ok(Function {
            sig,
            name,
            params,
            output,
            func,
        })
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.sig.eq(&(other.sig as u64))
    }
}

impl Eq for Function {}

impl PartialOrd for Function {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Function {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

/// This trait is used to enforce the rule that functions intended for cross-boundary calls must
/// have parameters which are either primitives implementing the `Type` trait or passable messages.
/// To be able to be a passable message, the type must
/// * Sized
/// * be `repr(C)` or `repr(transparent)` (where the single field must implement `Msg`)
pub trait Params: Sized {}

macro_rules! for_each_signature {
    ($mac:ident) => {
        $mac!();
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

macro_rules! impl_params {
    ($($t:ident)*) => (
        #[allow(unused_parens)]
        impl<$($t),*> Params for ($($t,)*)  where
            $($t: TypeSignature,)* {}
    );
}

for_each_signature!(impl_params);
