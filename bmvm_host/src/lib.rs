#![feature(new_range_api)]
#![feature(allocator_api)]
#![feature(iterator_try_collect)]
#![feature(adt_const_params)]

mod alloc;
mod elf;
pub mod linker;
mod runtime;
mod utils;
mod vm;

use bmvm_common::mem::{AddrSpace, Align, DefaultAddrSpace, Page4KiB, PhysAddr, align_floor};
use std::sync::OnceLock;

// re-export bmvm-common
pub use bmvm_common::TypeSignature;
pub use bmvm_common::hash::SignatureHasher;
pub use bmvm_common::mem;
pub use bmvm_common::registry;
pub use bmvm_common::vmi;
pub use bmvm_common::vmi::{ForeignShareable, OwnedShareable, Signature, Transport};

// re-export bmvm-macros
pub use bmvm_macros::{TypeSignature, expose_host as expose};

use crate::vm::{GDT_PAGE_REQUIRED, IDT_PAGE_REQUIRED};
pub use elf::Buffer;
pub use linker::hypercall::{CallableFunction, HypercallResult, WrapperFunc};
pub use runtime::*;
pub use vm::{Config, ConfigBuilder};

/// The default stack size for the guest (8MiB)
pub(crate) const GUEST_DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;
/// The default shared memory size (8MiB)
pub(crate) const DEFAULT_SHARED_MEMORY: usize = 8 * 1024 * 1024;

static ONCE_GUEST_SYSTEM_ADDR: OnceLock<PhysAddr> = OnceLock::new();
static ONCE_GUEST_STACK_ADDR: OnceLock<PhysAddr> = OnceLock::new();

#[allow(non_snake_case)]
#[inline]
pub(crate) fn GUEST_SYSTEM_ADDR() -> PhysAddr {
    *ONCE_GUEST_SYSTEM_ADDR.get_or_init(|| PhysAddr::new(1 << (DefaultAddrSpace::bits() - 1)))
}

#[allow(non_snake_case)]
#[inline]
pub(crate) fn GUEST_PAGING_ADDR() -> PhysAddr {
    GUEST_SYSTEM_ADDR()
        + (IDT_PAGE_REQUIRED * Page4KiB::ALIGNMENT as usize) as u64
        + (GDT_PAGE_REQUIRED * Page4KiB::ALIGNMENT as usize) as u64
}

#[allow(non_snake_case)]
#[inline]
pub(crate) fn GUEST_STACK_ADDR() -> PhysAddr {
    *ONCE_GUEST_STACK_ADDR
        .get_or_init(|| PhysAddr::new(align_floor(GUEST_SYSTEM_ADDR().as_u64() - 1)))
}

#[allow(unused_imports)]
mod test {
    use super::*;
    use bmvm_common::mem::VirtAddr;

    #[test]
    fn test_addr() {
        assert_eq!(
            GUEST_SYSTEM_ADDR(),
            PhysAddr::new(1 << (DefaultAddrSpace::bits() - 1))
        );
        assert_eq!(
            GUEST_STACK_ADDR(),
            PhysAddr::new(align_floor(GUEST_SYSTEM_ADDR().as_u64() - 1))
        );

        assert_eq!(
            GUEST_SYSTEM_ADDR().as_virt_addr(),
            VirtAddr::new(0xFFFF800000000000)
        )
    }
}
