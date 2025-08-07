use crate::linker::Func;
use bmvm_common::error::ExitCode;
use bmvm_common::mem::Transport;
use bmvm_common::vmi;
use bmvm_common::vmi::{FnCall, Signature};
use std::cmp::Ordering;
use std::ffi::IntoStringError;
use std::fmt::{Display, Formatter};

inventory::collect!(CallableFunction);

pub type HypercallResult = Result<Transport, ExitCode>;

pub type WrapperFunc = fn(Transport) -> HypercallResult;

pub struct CallableFunction {
    /// serialized FnCall
    pub meta: &'static [u8],
    /// Pointer to the wrapper function
    pub func: WrapperFunc,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub func: Func,
    pub call: WrapperFunc,
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.func)
    }
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
            func: Func {
                name,
                sig,
                params,
                output,
            },
            call: func,
        })
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        self.func.eq(&other.func)
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
        self.func.cmp(&other.func)
    }
}
