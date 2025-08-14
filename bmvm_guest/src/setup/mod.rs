use bmvm_common::error::ExitCode;
use bmvm_common::interprete::{Interpret, InterpretError};
use bmvm_common::mem::{Align, Arena, Flags, LayoutTable, Page4KiB};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, mem};
use core::arch::asm;

// Define the I/O port to write to (example: 0x3F8 for COM1)
pub(crate) const IO_PORT: u16 = 0x3f8;

/// write byte value to I/O port
pub(crate) fn write(port: u16, value: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        );
    }
}

/// Parse the memory info structure and initialize the paging system etc.
#[inline(always)]
pub fn setup() -> Result<(), ExitCode> {
    let raw_ptr = BMVM_MEM_LAYOUT_TABLE.as_u64() as *const u8;
    let raw = unsafe { core::slice::from_raw_parts(raw_ptr, Page4KiB::ALIGNMENT as usize) };
    let table = LayoutTable::from_bytes(raw).map_err(|interpret_err| match interpret_err {
        InterpretError::TooSmall(_, _) => ExitCode::InvalidMemoryLayoutTableTooSmall,
        InterpretError::Misaligned(_, _) => ExitCode::InvalidMemoryLayoutTableMisaligned,
    })?;

    let region_vmi_foreign = table
        .into_iter()
        .find(|entry| {
            entry
                .flags()
                .contains(Flags::PRESENT | Flags::DATA_SHARED_FOREIGN)
        })
        .ok_or(ExitCode::InvalidMemoryLayout)?;

    let region_vmi_owned = table
        .into_iter()
        .find(|entry| {
            entry
                .flags()
                .contains(Flags::PRESENT | Flags::DATA_SHARED_OWNED)
        })
        .ok_or(ExitCode::InvalidMemoryLayout)?;

    // stage 2 -> Layout parsed
    write(IO_PORT, 1);

    // set up the allocator for the VMI
    let foreign = Arena::from(region_vmi_foreign);
    let owned = Arena::from(region_vmi_owned);
    mem::init(owned, foreign);

    write(IO_PORT, 2);

    Ok(())
}
