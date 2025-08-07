use crate::linker::{Func, compute_signature};
use bmvm_common::TypeSignature;
use bmvm_common::mem::ForeignShareable;
use bmvm_common::registry::Params;

pub type Function = Func;

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
            sig,
            name,
            params,
            output,
        }
    }
}
