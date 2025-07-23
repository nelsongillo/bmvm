use crate::exit_with_code;
use bmvm_common::error::ExitCode;
use bmvm_common::vmi::{Signature, upcalls};

pub fn execute(sig: Signature) {
    let calls = upcalls();
    match calls.binary_search_by_key(&sig, |upcall| upcall.sig) {
        Ok(idx) => {
            let upcall = &calls[idx];
            (upcall.func)();
        }
        Err(_) => exit_with_code(ExitCode::UnknownUpcall(sig)),
    }
}
