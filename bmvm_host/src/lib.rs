mod alloc;
mod elf;
mod linker;
mod module;
mod runtime;
mod setup;
mod utils;

use bmvm_common::mem::{DefaultAlign, PhysAddr, align_ceil};
#[cfg(feature = "std")]
pub use bmvm_common::meta;
#[cfg(feature = "std")]
pub use bmvm_common::registry;
pub use bmvm_macros::expose_guest as expose;

/// The default stack size for the guest (8MiB)
pub(crate) const BMVM_GUEST_DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;
pub(crate) const BMVM_GUEST_SYSTEM: PhysAddr = PhysAddr::new(0x1000_0000_0000);
/// The temporary system region size (1MiB)
pub(crate) const BMVM_GUEST_TMP_SYSTEM_SIZE: u64 = align_ceil(1 * 1024 * 1024 * 1024);
