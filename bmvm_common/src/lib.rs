#![cfg_attr(not(feature = "std"), no_std)]
#![feature(alloc_error_handler)]
extern crate alloc;

use crate::mem::{PhysAddr, VirtAddr};

pub mod hash;
pub mod interprete;

pub mod error;
pub mod mem;
#[cfg(feature = "std")]
pub mod meta;
pub mod registry;


/// The ELF section name for the metadata containing the call guest provided function information.
pub const BMVM_META_SECTION: &str = ".bmvm.call.host";

/// The memory layout table will be places at this address for the guest to access.
pub const BMVM_MEM_LAYOUT_TABLE: PhysAddr = PhysAddr::new(0x0000);

/// The temporary global descriptor table (GDT) used for setting up long mode will be placed at this
/// address. The guest can either modify this table or create another one and switch later.
pub const BMVM_TMP_GDT: PhysAddr = PhysAddr::new(0x1000);

/// The temporary interrupt descriptor table (IDT) used for setting up long mode will be placed at
/// this address. The guest can either modify this table or create another one and switch later.
pub const BMVM_TMP_IDT: PhysAddr = PhysAddr::new(0x2000);

/// The temporary paging tables will be placed at this address. The host will initialize the tables
/// to set up long-mode for the guest. A very rough structure is provided, but is intended to
/// be replaced by the guest (optionally at a different, as this memory region is not write
/// protected against the host)
pub const BMVM_TMP_PAGING: PhysAddr = PhysAddr::new(0x3000);

/// The beginning of the .text segment should be at least 0x400000. This is similar to the x86_64
/// convention (https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf).
pub const BMVM_MIN_TEXT_SEGMENT: PhysAddr = PhysAddr::new(0x400000);
