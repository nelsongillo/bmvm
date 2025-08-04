#![cfg_attr(not(feature = "std"), no_std)]
#![feature(alloc_error_handler)]
#![feature(sync_unsafe_cell)]
#![feature(allocator_api)]
#![feature(slice_as_array)]
#[cfg(feature = "std")]
extern crate alloc;
extern crate core;

#[cfg(all(feature = "vmi-consume", feature = "vmi-execute"))]
compile_error!("Features `vmi-consume` and `vmi-execute` cannot be enabled at the same time.");

pub mod error;
pub mod hash;
pub mod interprete;
pub mod mem;
pub mod registry;
mod typesignature;
pub mod vmi;

use crate::mem::PhysAddr;
pub use crate::typesignature::TypeSignature;

/// The IO Port used for triggering hypercalls to host from the guest.
pub const HYPERCALL_IO_PORT: u16 = 0x0434;

/// The ELF section name for the metadata containing the call guest required function information.
pub const BMVM_META_SECTION_HOST: &str = ".bmvm.vmi.host";
/// The ELF section name for the metadata containing the call guest provided function information.
pub const BMVM_META_SECTION_EXPOSE: &str = ".bmvm.vmi.expose";
/// The ELF section name for the metadata containing the call guest required function calls.
pub const BMVM_META_SECTION_EXPOSE_CALLS: &str = ".bmvm.vmi.expose.calls";
/// The ELF section name for the debug metadata.
pub const BMVM_META_SECTION_DEBUG: &str = ".bmvm.vmi.debug";

/// The address where the temporary system region should be mapped into the guest
pub const BMVM_TMP_SYS: PhysAddr = PhysAddr::new_unchecked(0x1000);

/// The memory layout table will be places at this address for the guest to access.
pub const BMVM_MEM_LAYOUT_TABLE: PhysAddr = PhysAddr::new_unchecked(0x1000);

/// The temporary global descriptor table (GDT) used for setting up long mode will be placed at this
/// address. The guest can either modify this table or create another one and switch later.
pub const BMVM_TMP_GDT: PhysAddr = PhysAddr::new_unchecked(0x3000);
pub const BMVM_TMP_GDT_LIMIT: usize = 0x1000;

/// The temporary interrupt descriptor table (IDT) used for setting up long mode will be placed at
/// this address. The guest can either modify this table or create another one and switch later.
pub const BMVM_TMP_IDT: PhysAddr = PhysAddr::new_unchecked(0x2000);

/// The temporary paging tables will be placed at this address. The host will initialize the tables
/// to set up long-mode for the guest. A very rough structure is provided, but is intended to
/// be replaced by the guest (optionally at a different, as this memory region is not write
/// protected against the host)
pub const BMVM_TMP_PAGING: PhysAddr = PhysAddr::new_unchecked(0x4000);

pub const fn region_abs_offset(addr: u64) -> u64 {
    addr - BMVM_TMP_SYS.as_u64()
}
