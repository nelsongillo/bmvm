mod alloc;
mod elf;
mod linker;
mod runtime;
mod utils;
mod vm;

use bmvm_common::mem::{AddrSpace, DefaultAddrSpace, align_floor};
pub use bmvm_common::mem::{PhysAddr, align_ceil};
pub use bmvm_common::meta;
pub use bmvm_common::registry;
pub use bmvm_macros::expose_guest as expose;
pub use runtime::*;
use std::sync::OnceLock;

/// The default stack size for the guest (8MiB)
pub(crate) const GUEST_DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;
/// The temporary system region size (1MiB)
pub(crate) const GUEST_TMP_SYSTEM_SIZE: u64 = 1 * 1024 * 1024;

/// The beginning of the .text segment should be at least 0x400000. This is similar to the x86_64
/// convention (https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf).
pub(crate) const MIN_TEXT_SEGMENT: u64 = 0x400000;

#[allow(non_snake_case)]
#[inline]
pub(crate) fn GUEST_SYSTEM_ADDR() -> PhysAddr {
    *ONCE_GUEST_SYSTEM_ADDR.get_or_init(|| PhysAddr::new(1 << (DefaultAddrSpace::bits() - 1)))
}

#[allow(non_snake_case)]
#[inline]
pub(crate) fn GUEST_STACK_ADDR() -> PhysAddr {
    *ONCE_GUEST_STACK_ADDR
        .get_or_init(|| PhysAddr::new(align_floor(GUEST_SYSTEM_ADDR().as_u64() - 1)))
}

static ONCE_GUEST_SYSTEM_ADDR: OnceLock<PhysAddr> = OnceLock::new();
static ONCE_GUEST_STACK_ADDR: OnceLock<PhysAddr> = OnceLock::new();
