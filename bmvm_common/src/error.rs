use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::PageSize;

pub enum ExitCode {
    Normal = 0,
    InvalidMemoryLayoutTable = 1,
    InvalidMemoryLayout = 2,
    /// An additional frame was needed for the mapping process, but the frame allocator
    /// returned `None`.
    FrameAllocationFailed = 3,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of an already mapped huge page.
    ParentEntryHugePage = 4,
    /// The given page is already mapped to a physical frame.
    PageAlreadyMapped = 5,
    Unknown = 255
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
