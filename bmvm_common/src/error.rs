use x86_64::structures::paging::PageSize;
use x86_64::structures::paging::mapper::MapToError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Normal,
    InvalidMemoryLayoutTableTooSmall,
    InvalidMemoryLayoutTableMisaligned,
    InvalidMemoryLayout,
    /// An additional frame was needed for the mapping process, but the frame allocator
    /// returned `None`.
    FrameAllocationFailed,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of an already mapped huge page.
    ParentEntryHugePage,
    /// The given page is already mapped to a physical frame.
    PageAlreadyMapped,
    Unmapped(u8),
}

impl ExitCode {
    pub fn as_u8(self) -> u8 {
        self.into()
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
            1 => ExitCode::InvalidMemoryLayoutTableTooSmall,
            2 => ExitCode::InvalidMemoryLayoutTableMisaligned,
            3 => ExitCode::InvalidMemoryLayout,
            4 => ExitCode::FrameAllocationFailed,
            5 => ExitCode::ParentEntryHugePage,
            6 => ExitCode::PageAlreadyMapped,
            _ => ExitCode::Unmapped(value),
        }
    }
}

impl Into<u8> for ExitCode {
    fn into(self) -> u8 {
        match self {
            ExitCode::Normal => 0,
            ExitCode::InvalidMemoryLayoutTableTooSmall => 1,
            ExitCode::InvalidMemoryLayoutTableMisaligned => 2,
            ExitCode::InvalidMemoryLayout => 3,
            ExitCode::FrameAllocationFailed => 4,
            ExitCode::ParentEntryHugePage => 5,
            ExitCode::PageAlreadyMapped => 6,
            ExitCode::Unmapped(value) => value,
        }
    }
}
