use crate::mem::{RawOffsetPtr, VirtAddr};
use crate::vmi::Signature;

#[cfg_attr(
    feature = "vmi-consume",
    derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)
)]
pub enum ExitCode {
    /// Program exited normally
    #[cfg_attr(feature = "vmi-consume", error("Normal Exit"))]
    Normal,
    /// Setup complete, ready to execute functions
    #[cfg_attr(feature = "vmi-consume", error("Ready"))]
    Ready,
    #[cfg_attr(feature = "vmi-consume", error("Return"))]
    Return,
    /// An invalid offset pointer was provided
    #[cfg_attr(feature = "vmi-consume", error("Invalid offset pointer {0:x}"))]
    Ptr(RawOffsetPtr),
    #[cfg_attr(feature = "vmi-consume", error("Null pointer"))]
    NullPtr,
    /// Allocator initialization failed
    #[cfg_attr(feature = "vmi-consume", error("Allocator initialization failed"))]
    AllocatorInitFailed,
    /// Allocation failed
    #[cfg_attr(feature = "vmi-consume", error("Allocation failed"))]
    AllocationFailed,
    /// The provided layout table was too small
    #[cfg_attr(
        feature = "vmi-consume",
        error("The provided layout table was too small")
    )]
    InvalidMemoryLayoutTableTooSmall,
    /// The pointer to the layout table was misaligned
    #[cfg_attr(
        feature = "vmi-consume",
        error("The pointer to the layout table was misaligned")
    )]
    InvalidMemoryLayoutTableMisaligned,
    /// The provided layout table is invalid
    #[cfg_attr(feature = "vmi-consume", error("The provided layout table is invalid"))]
    InvalidMemoryLayout,
    /// An additional frame was needed for the mapping process, but the frame allocator
    /// returned `None`.
    #[cfg_attr(
        feature = "vmi-consume",
        error(
            "An additional frame was needed for the mapping process, but the frame allocator returned `None`."
        )
    )]
    FrameAllocationFailed,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of an already mapped huge page.
    #[cfg_attr(
        feature = "vmi-consume",
        error("Page already part of a huge page due to set flag in parent")
    )]
    ParentEntryHugePage,
    /// The given page is already mapped to a physical frame.
    #[cfg_attr(feature = "vmi-consume", error("Page already mapped"))]
    PageAlreadyMapped,
    /// The upcall signature is not known.
    #[cfg_attr(
        feature = "vmi-consume",
        error("Tried to call unknown function with signature: {0}")
    )]
    UnknownUpcall(Signature),
    /// The provided buffer capacity is Zero
    #[cfg_attr(feature = "vmi-consume", error("Buffer capacity is ZERO"))]
    ZeroCapacity,
    /// The given exit code is not mapped to an enum variant.
    #[cfg_attr(feature = "vmi-consume", error("Panic"))]
    Panic(VirtAddr),
    /// The given exit code is not mapped to an enum variant.
    #[cfg_attr(feature = "vmi-consume", error("Unmapped exit code: {0}"))]
    Unmapped(u8),
    /// Guest Interrupt Handler triggered
    #[cfg_attr(feature = "vmi-consume", error("Guest Interrupt: {0}"))]
    Interrupt(u8),
}

impl ExitCode {
    pub const fn as_u8(self) -> u8 {
        match self {
            ExitCode::Normal => 0,
            ExitCode::Ready => 1,
            ExitCode::Return => 2,
            ExitCode::Ptr(_) => 3,
            ExitCode::NullPtr => 4,
            ExitCode::AllocatorInitFailed => 5,
            ExitCode::AllocationFailed => 6,
            ExitCode::InvalidMemoryLayoutTableTooSmall => 7,
            ExitCode::InvalidMemoryLayoutTableMisaligned => 8,
            ExitCode::InvalidMemoryLayout => 9,
            ExitCode::FrameAllocationFailed => 10,
            ExitCode::ParentEntryHugePage => 11,
            ExitCode::PageAlreadyMapped => 12,
            ExitCode::UnknownUpcall(_) => 13,
            ExitCode::ZeroCapacity => 14,
            ExitCode::Interrupt(_) => 15,
            ExitCode::Panic(_) => 254,
            ExitCode::Unmapped(value) => value,
        }
    }
}

#[cfg(feature = "vmi-execute")]
impl ExitCode {
    /// Write additional values to registers before VM exit.
    pub fn write_values(self) {
        unsafe {
            match self {
                ExitCode::UnknownUpcall(sig) => core::arch::asm!("mov rbx, {}", in(reg) sig),
                ExitCode::Unmapped(code) => core::arch::asm!("mov bl, {}", in(reg_byte) code),
                ExitCode::Ptr(ptr) => core::arch::asm!("mov ebx, {0:e}", in(reg) ptr.as_u32()),
                ExitCode::Panic(addr) => core::arch::asm!("mov rbx, {0}", in(reg) addr.as_u64()),
                ExitCode::Interrupt(index) => {
                    core::arch::asm!("mov rbx, {0}", in(reg) index as u64)
                }
                _ => {}
            }
        }
    }
}

#[cfg(feature = "vmi-consume")]
impl ExitCode {
    /// Read additional values from registers after VM exit.
    pub fn read_values(self, regs: &kvm_bindings::kvm_regs) -> Self {
        match self {
            ExitCode::Ptr(_) => {
                let ptr: RawOffsetPtr = RawOffsetPtr::from(regs.rbx as u32);
                ExitCode::Ptr(ptr)
            }
            ExitCode::UnknownUpcall(_) => {
                let sig: Signature = regs.rbx;
                ExitCode::UnknownUpcall(sig)
            }
            ExitCode::Panic(_) => {
                let addr: VirtAddr = VirtAddr::new(regs.rbx);
                ExitCode::Panic(addr)
            }
            ExitCode::Interrupt(_) => ExitCode::Interrupt(regs.rbx as u8),
            ExitCode::Unmapped(_) => {
                let code: u8 = (regs.rbx & 0xFF) as u8;
                ExitCode::Unmapped(code)
            }
            _ => self,
        }
    }
}

impl From<u8> for ExitCode {
    fn from(value: u8) -> Self {
        match value {
            0 => ExitCode::Normal,
            1 => ExitCode::Ready,
            2 => ExitCode::Return,
            3 => ExitCode::Ptr(RawOffsetPtr::from(value as u32)),
            4 => ExitCode::NullPtr,
            5 => ExitCode::AllocatorInitFailed,
            6 => ExitCode::AllocationFailed,
            7 => ExitCode::InvalidMemoryLayoutTableTooSmall,
            8 => ExitCode::InvalidMemoryLayoutTableMisaligned,
            9 => ExitCode::InvalidMemoryLayout,
            10 => ExitCode::FrameAllocationFailed,
            11 => ExitCode::ParentEntryHugePage,
            12 => ExitCode::PageAlreadyMapped,
            13 => ExitCode::UnknownUpcall(Signature::from(value)),
            14 => ExitCode::ZeroCapacity,
            15 => ExitCode::Interrupt(0),
            254 => ExitCode::Panic(VirtAddr::new_unchecked(value as u64)),
            v => ExitCode::Unmapped(v),
        }
    }
}

impl From<ExitCode> for u8 {
    fn from(code: ExitCode) -> u8 {
        match code {
            ExitCode::Normal => 0,
            ExitCode::Ready => 1,
            ExitCode::Return => 2,
            ExitCode::Ptr(_) => 3,
            ExitCode::NullPtr => 4,
            ExitCode::AllocatorInitFailed => 5,
            ExitCode::AllocationFailed => 6,
            ExitCode::InvalidMemoryLayoutTableTooSmall => 7,
            ExitCode::InvalidMemoryLayoutTableMisaligned => 8,
            ExitCode::InvalidMemoryLayout => 9,
            ExitCode::FrameAllocationFailed => 10,
            ExitCode::ParentEntryHugePage => 11,
            ExitCode::PageAlreadyMapped => 12,
            ExitCode::UnknownUpcall(_) => 13,
            ExitCode::ZeroCapacity => 14,
            ExitCode::Interrupt(_) => 15,
            ExitCode::Panic(_) => 254,
            ExitCode::Unmapped(value) => value,
        }
    }
}
