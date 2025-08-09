use bmvm_common::BMVM_MEM_LAYOUT_TABLE;
use bmvm_common::error::ExitCode;
use bmvm_common::interprete::{Interpret, InterpretError};
use bmvm_common::mem::{Align, Flags, LayoutTable, Page4KiB};
use core::arch::asm;

mod gdt;
mod idt;
mod paging;

/// GDT requires 1 page
pub const GDT_PAGE_REQ: u64 = 1;
pub const GDT_SPACE_REQ: u64 = Page4KiB::ALIGNMENT * GDT_PAGE_REQ;

/// IDT requires 1 page
pub const IDT_PAGE_REQ: u64 = 1;
pub const IDT_SPACE_REQ: u64 = Page4KiB::ALIGNMENT * IDT_PAGE_REQ;

/// Pages to allocate for non-paging related structures in the sys memory region
pub const NON_PAGING_PAGE_REQ: u64 = GDT_PAGE_REQ + IDT_PAGE_REQ;
/// Bytes offset the paging structure by in the sys memory region
pub const NON_PAGING_SPACE_REQ: u64 = GDT_SPACE_REQ + IDT_SPACE_REQ;

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

/*
 TODO:
    * Initialize Allocators (foreign and owned)
*/

/// Parse the memory info structure and initialize the paging system etc.
#[inline(always)]
pub fn setup() -> Result<(), ExitCode> {
    let raw_ptr = BMVM_MEM_LAYOUT_TABLE.as_u64() as *const u8;
    let raw = unsafe { core::slice::from_raw_parts(raw_ptr, Page4KiB::ALIGNMENT as usize) };
    let table = LayoutTable::from_bytes(raw).map_err(|interpret_err| match interpret_err {
        InterpretError::TooSmall(_, _) => ExitCode::InvalidMemoryLayoutTableTooSmall,
        InterpretError::Misaligned(_, _) => ExitCode::InvalidMemoryLayoutTableMisaligned,
    })?;

    // stage 1 -> Layout parsed
    write(IO_PORT, 0);

    let region_sys = table
        .into_iter()
        .find(|entry| entry.flags().contains(Flags::PRESENT | Flags::SYSTEM))
        .ok_or(ExitCode::InvalidMemoryLayout)?;

    // stage 2 -> Layout parsed
    write(IO_PORT, 1);

    // set up the Interrupt Table
    idt::setup(&region_sys, 0)?;

    // stage 3 -> IDT done
    write(IO_PORT, 2);

    // set up the Global Descriptor Table
    gdt::setup(&region_sys, IDT_SPACE_REQ)?;

    // stage 4 -> GDT done
    write(IO_PORT, 3);

    // set up the paging structure
    paging::setup(table, region_sys)?;

    // stage 5 -> Paging Done
    write(IO_PORT, 4);

    Ok(())
}
