mod hypercall;
mod upcall;

use bmvm_common::TypeSignature;
use bmvm_common::interprete::Interpret;
pub use hypercall::*;
pub use upcall::*;

pub const HYPERCALL_IO_PORT: u16 = 0x0434;

pub use hypercall::execute as hypercall;
// pub use upcall::execute as upcall;
