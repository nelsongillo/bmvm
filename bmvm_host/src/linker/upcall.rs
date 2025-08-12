use crate::linker::{Func, compute_signature};
use bmvm_common::TypeSignature;
use bmvm_common::registry::Params;
use bmvm_common::vmi::{FnPtr, ForeignShareable};

#[derive(Debug, Clone)]
pub struct Function {
    pub(crate) base: Func,
    pub(super) ptr: Option<FnPtr>,
}

impl Function {
    pub fn new<P, R>(name: &'static str) -> Self
    where
        P: Params,
        R: ForeignShareable,
    {
        let sig = compute_signature::<P, R>(name);
        let name = String::from(name);
        let params = P::strings();
        let output = if R::SIGNATURE != <() as TypeSignature>::SIGNATURE {
            Some(R::name())
        } else {
            None
        };

        Function {
            base: Func {
                sig,
                name,
                params,
                output,
            },
            ptr: None,
        }
    }

    pub fn link(&mut self, ptr: FnPtr) {
        self.ptr = Some(ptr);
    }

    pub fn ptr(&self) -> Option<FnPtr> {
        self.ptr
    }
}
