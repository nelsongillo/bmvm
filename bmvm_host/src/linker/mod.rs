mod config;
pub mod hypercall;
mod linker;
pub mod upcall;

use crate::linker::hypercall::Function;
use bmvm_common::TypeSignature;
use bmvm_common::hash::SignatureHasher;
use bmvm_common::mem::ForeignShareable;
use bmvm_common::registry::Params;
use bmvm_common::vmi::Signature;
pub use config::*;
pub use linker::*;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

const fn compute_signature<P, R>(func: &'static str) -> u64
where
    P: Params,
    R: ForeignShareable,
{
    let mut hasher = SignatureHasher::new();
    hasher.write(func.as_bytes());
    hasher.write(<P as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
    hasher.write(<R as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
    hasher.finish()
}

#[derive(Clone, Debug)]
pub struct Func {
    pub sig: Signature,
    pub name: String,
    pub params: Vec<String>,
    pub output: Option<String>,
}

impl PartialEq for Func {
    fn eq(&self, other: &Self) -> bool {
        self.sig.eq(&(other.sig as u64))
    }
}

impl Eq for Func {}

impl PartialOrd for Func {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Func {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Display for Func {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let result = self.output.as_ref().map(|o| format!(" -> {}", o));
        write!(
            f,
            "{}({}){}",
            self.name,
            self.params.join(", "),
            result.unwrap_or_default()
        )
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum CallDirection {
    HostToGuest,
    GuestToHost,
}

impl Display for CallDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CallDirection::HostToGuest => write!(f, "Host -> Guest"),
            CallDirection::GuestToHost => write!(f, "Guest -> Host"),
        }
    }
}
