/*
TODO:
 - setup idt
 - setup gdt
 - setup stack
 - setup host-calls
 */
use bmvm_common::error::ExitCode;
use bmvm_common::interprete::Interpret;
use bmvm_common::mem::{Flags, LayoutTable, LayoutTableEntry};
use core::ops::BitAnd;

mod gdt;
mod idt;
mod paging;

/// The address of the memory info structure, which must be present on initialization.
/// The info table will span at max 1 page
const MEM_INFO_ADDR: u64 = 0x1000;

/// Parse the memory info structure and initialize the paging system etc.
#[inline(always)]
pub(crate) fn setup() -> Result<(), ExitCode> {
    let raw = unsafe { core::slice::from_raw_parts(MEM_INFO_ADDR as *const u8, 1024) };
    let table = LayoutTable::from_bytes(raw).map_err(|_| ExitCode::InvalidMemoryLayoutTable)?;

    let region_sys: &LayoutTableEntry = table
        .entries
        .iter()
        .find(|entry| entry.is_present() && entry.flags().bitand(Flags::SYSTEM).bits() > 0)
        .ok_or(ExitCode::InvalidMemoryLayout)?;

    // set up the Interrupt Table
    idt::setup()?;

    // set up the Global Descriptor Table
    gdt::setup()?;

    // set up the paging structure
    paging::setup(table, region_sys.clone())?;

    Ok(())
}
