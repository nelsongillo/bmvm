use bmvm_common::error::ExitCode;
use bmvm_common::interprete::{Interpret, InterpretError};
use bmvm_common::mem::{Align, Arena, DataAccessMode, LayoutTable, Page4KiB};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, mem};

/// Parse the memory info structure and initialize the paging system etc.
#[inline(never)]
pub(super) fn setup() -> Result<(), ExitCode> {
    let raw_ptr = BMVM_MEM_LAYOUT_TABLE.as_u64() as *const u8;
    let raw = unsafe { core::slice::from_raw_parts(raw_ptr, Page4KiB::ALIGNMENT as usize) };
    let table = LayoutTable::from_bytes(raw).map_err(|interpret_err| match interpret_err {
        InterpretError::TooSmall(_, _) => ExitCode::InvalidMemoryLayoutTableTooSmall,
        InterpretError::Misaligned(_, _) => ExitCode::InvalidMemoryLayoutTableMisaligned,
    })?;

    let shared = table
        .into_iter()
        .find(|entry| {
            entry
                .flags()
                .data_access_mode()
                .is_some_and(|m| m == DataAccessMode::Shared)
        })
        .map(Arena::from);

    // set up the allocator for the VMI
    mem::init(shared);

    Ok(())
}
