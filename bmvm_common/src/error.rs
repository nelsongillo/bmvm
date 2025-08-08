use crate::mem::RawOffsetPtr;
use crate::vmi::Signature;
use x86_64::structures::paging::PageSize;
use x86_64::structures::paging::mapper::MapToError;

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
    /// An invalid offset pointer was provided
    #[cfg_attr(feature = "vmi-consume", error("Invalid offset pointer {0:x}"))]
    Ptr(RawOffsetPtr),
    /// Allocation failed
    #[cfg_attr(feature = "vmi-consume", error("Allocation failed"))]
    AllocatorFailed,
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
    #[cfg_attr(feature = "vmi-consume", error("Unmapped exit code: {0}"))]
    Unmapped(u8),
}

impl ExitCode {
    pub fn as_u8(self) -> u8 {
        self.into()
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
            ExitCode::UnknownUpcall(_) => {
                let sig: Signature = regs.rbx;
                ExitCode::UnknownUpcall(sig)
            }
            ExitCode::Unmapped(_) => {
                let code: u8 = (regs.rbx & 0xFF) as u8;
                ExitCode::Unmapped(code)
            }
            _ => self,
        }
    }
}

impl<S: PageSize> From<MapToError<S>> for ExitCode {
    fn from(value: MapToError<S>) -> Self {
        match value {
            MapToError::FrameAllocationFailed => ExitCode::FrameAllocationFailed,
            MapToError::ParentEntryHugePage => ExitCode::ParentEntryHugePage,
            MapToError::PageAlreadyMapped(_) => ExitCode::PageAlreadyMapped,
        }
    }
}

impl From<u8> for ExitCode {
    fn from(value: u8) -> Self {
        match value {
            0 => ExitCode::Normal,
            1 => ExitCode::Ready,
            2 => ExitCode::Ptr(RawOffsetPtr::from(value as u32)),
            3 => ExitCode::AllocatorFailed,
            4 => ExitCode::InvalidMemoryLayoutTableTooSmall,
            5 => ExitCode::InvalidMemoryLayoutTableMisaligned,
            6 => ExitCode::InvalidMemoryLayout,
            7 => ExitCode::FrameAllocationFailed,
            8 => ExitCode::ParentEntryHugePage,
            9 => ExitCode::PageAlreadyMapped,
            10 => ExitCode::UnknownUpcall(Signature::from(value)),
            11 => ExitCode::ZeroCapacity,
            v => ExitCode::Unmapped(v),
        }
    }
}

impl Into<u8> for ExitCode {
    fn into(self) -> u8 {
        match self {
            ExitCode::Normal => 0,
            ExitCode::Ready => 1,
            ExitCode::Ptr(_) => 2,
            ExitCode::AllocatorFailed => 3,
            ExitCode::InvalidMemoryLayoutTableTooSmall => 4,
            ExitCode::InvalidMemoryLayoutTableMisaligned => 5,
            ExitCode::InvalidMemoryLayout => 6,
            ExitCode::FrameAllocationFailed => 7,
            ExitCode::ParentEntryHugePage => 8,
            ExitCode::PageAlreadyMapped => 9,
            ExitCode::UnknownUpcall(_) => 10,
            ExitCode::ZeroCapacity => 11,
            ExitCode::Unmapped(value) => value,
        }
    }
}
