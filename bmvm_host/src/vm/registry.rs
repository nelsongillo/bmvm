use crate::linker::compute_signature;
use crate::linker::hypercall;
use crate::linker::upcall;
use bmvm_common::error::ExitCode;
use bmvm_common::mem;
use bmvm_common::registry::Params;
use bmvm_common::vmi::{ForeignShareable, Signature, Transport};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Guest tried to call an unknown function: {0}")]
    UnknownFunction(Signature),
    #[error("Guest tried to call an unliked upcall: {0}")]
    UnlikedUpcall(Signature),
    #[error("Hypercall threw an error: {0}")]
    HypercallExec(ExitCode),
    #[error("Unable to pass arguments to guest: {0}")]
    UpcallParam(mem::Error),
    #[error("Upcall execution threw an error: {0}")]
    UpcallExec(ExitCode),
}

pub(super) struct Hypercalls {
    inner: Vec<hypercall::Function>,
}

impl Default for Hypercalls {
    fn default() -> Self {
        Hypercalls::from(Vec::default())
    }
}

impl Hypercalls {
    pub fn try_execute(&self, sig: Signature, transport: Transport) -> Result<Transport> {
        let idx = match self.inner.binary_search_by_key(&sig, |f| f.func.sig) {
            Ok(idx) => idx,
            Err(_) => return Err(Error::UnknownFunction(sig)),
        };

        let func = self.inner[idx].call;
        let output = func(transport).map_err(Error::HypercallExec)?;
        Ok(output)
    }
}

impl From<Vec<hypercall::Function>> for Hypercalls {
    fn from(mut functions: Vec<hypercall::Function>) -> Self {
        functions.sort_by_key(|f| f.func.sig);
        Self { inner: functions }
    }
}

pub(super) struct Upcalls {
    inner: Vec<upcall::Function>,
}

impl Default for Upcalls {
    fn default() -> Self {
        Upcalls::from(Vec::default())
    }
}

impl Upcalls {
    pub fn find_upcall<P, R>(&self, name: &'static str) -> Result<&upcall::Function>
    where
        P: Params,
        R: ForeignShareable,
    {
        let sig = compute_signature::<P, R>(name);
        let idx = match self.inner.binary_search_by_key(&sig, |f| f.base.sig) {
            Ok(idx) => idx,
            Err(_) => return Err(Error::UnknownFunction(sig)),
        };

        let func = &self.inner[idx];
        let _ = func.ptr().ok_or(Error::UnlikedUpcall(sig))?;
        Ok(&self.inner[idx])
    }
}

impl From<Vec<upcall::Function>> for Upcalls {
    fn from(mut functions: Vec<upcall::Function>) -> Self {
        functions.sort_by_key(|f| f.base.sig);
        Self { inner: functions }
    }
}
