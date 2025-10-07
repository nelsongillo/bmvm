#![cfg_attr(not(any(feature = "vmi-consume", feature = "vmi-macro")), no_std)]
#![feature(allocator_api)]
#![feature(macro_metavar_expr_concat)]
#[cfg(feature = "vmi-consume")]
extern crate alloc;
extern crate core;

#[cfg(all(feature = "vmi-consume", feature = "vmi-execute"))]
compile_error!("Features `vmi-consume` and `vmi-execute` cannot be enabled at the same time.");

pub mod error;
pub mod hash;
pub mod interprete;
pub mod mem;
#[cfg(feature = "vmi-consume")]
pub mod registry;
mod typesignature;
pub mod vmi;

use crate::mem::PhysAddr;
pub use crate::typesignature::TypeSignature;

/// The IO Port used for triggering hypercalls to host from the guest.
pub const HYPERCALL_IO_PORT: u16 = 0x0434;
/// The IO Port used for exiting from the guest to host with an ExitCode.
pub const EXIT_IO_PORT: u16 = 0x0433;

/// The ELF section name for the metadata containing the call guest required function information.
pub const BMVM_META_SECTION_HOST: &str = ".bmvm.vpc.hypercall";
/// The ELF section name for the metadata containing the call guest provided function information.
pub const BMVM_META_SECTION_EXPOSE: &str = ".bmvm.vpc.upcall";
/// The ELF section name for the metadata containing the call guest required function calls.
pub const BMVM_META_SECTION_EXPOSE_CALLS: &str = ".bmvm.vpc.upcall.calls";
/// The ELF section name for the debug metadata.
pub const BMVM_META_SECTION_DEBUG: &str = ".bmvm.vpc.debug";
/// The memory layout table will be places at this address for the guest to access.
pub const BMVM_MEM_LAYOUT_TABLE: PhysAddr = PhysAddr::new_unchecked(0x1000);
