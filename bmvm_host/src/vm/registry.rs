use crate::linker::hypercall::Function;
use bmvm_common::error::ExitCode;
use bmvm_common::mem::Transport;
use bmvm_common::vmi::Signature;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Guest tried to call an unknown function: {0}")]
    UnknownFunction(Signature),
    #[error("Hypercall threw an error: {0}")]
    ExecError(#[from] ExitCode),
}

pub(super) struct FunctionRegistry {
    inner: Vec<Function>,
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        FunctionRegistry::from(Vec::default())
    }
}

impl FunctionRegistry {
    pub fn try_execute(&self, sig: Signature, transport: Transport) -> Result<Transport> {
        let idx = match self.inner.binary_search_by_key(&sig, |f| f.sig) {
            Ok(idx) => idx,
            Err(_) => return Err(Error::UnknownFunction(sig)),
        };

        let func = self.inner[idx].func;
        let output = func(transport)?;
        Ok(output)
    }
}

impl From<Vec<Function>> for FunctionRegistry {
    fn from(functions: Vec<Function>) -> Self {
        Self { inner: functions }
    }
}
