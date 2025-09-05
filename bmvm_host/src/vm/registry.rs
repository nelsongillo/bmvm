use crate::linker::compute_signature;
use crate::linker::hypercall;
use crate::linker::upcall;
use bmvm_common::error::ExitCode;
use bmvm_common::mem;
use bmvm_common::registry::Params;
use bmvm_common::vmi::{ForeignShareable, Signature, Transport};
use rustc_hash::FxHashMap;

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

#[derive(Debug)]
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

#[derive(Debug)]
pub(super) struct Upcalls {
    inner: FxHashMap<Signature, upcall::Function>,
}

impl Default for Upcalls {
    fn default() -> Self {
        Upcalls::from(Vec::default())
    }
}

impl Upcalls {
    #[inline]
    pub fn find_upcall<P, R>(&self, name: &'static str) -> Result<&upcall::Function>
    where
        P: Params,
        R: ForeignShareable,
    {
        let sig: u64 = compute_signature::<P, R>(name);
        let func = match self.inner.get(&sig) {
            Some(idx) => idx,
            None => return Err(Error::UnknownFunction(sig)),
        };
        func.ptr().ok_or_else(|| Error::UnlikedUpcall(sig))?;

        Ok(func)
    }
}

impl From<Vec<upcall::Function>> for Upcalls {
    fn from(mut functions: Vec<upcall::Function>) -> Self {
        let mut map = FxHashMap::with_capacity_and_hasher(functions.len(), Default::default());
        for func in functions.drain(..) {
            map.insert(func.base.sig, func);
        }
        Self { inner: map }
    }
}
