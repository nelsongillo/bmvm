use crate::BMVM_GUEST_TMP_SYSTEM_SIZE;
use crate::alloc::{Manager, ReadWrite};
use crate::elf::ExecBundle;
use crate::utils::estimate_paging_size_requirements;
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, BMVM_TMP_GDT, BMVM_TMP_IDT, BMVM_TMP_PAGING};
use bmvm_common::interprete::Interpret;
use bmvm_common::mem::{LayoutTable, LayoutTableEntry};

const NUM_SYS_BUFFER_PAGES: u64 = 4;
const PAGE_TABLE_SIZE: u64 = 0x1000;
const IDT_SIZE: u64 = 0x1000;
const GDT_SIZE: u64 = 0x1000;
const IDT_LOCATION: u64 = 0x0000;
const GDT_LOCATION: u64 = 0x1000;
const PAGING_LOCATION: u64 = 0x2000;

fn setup_longmode(exec: &ExecBundle, manager: impl Manager) -> anyhow::Result<()> {
    let mut layout_region = manager.allocate::<ReadWrite>(size_of::<LayoutTable>() as u64)?;
    let layout = LayoutTable::from_mut_bytes(layout_region.as_mut())?;

    layout.entries[0] = estimate_sys_region(&exec.layout);
    let mut idx = 1;
    for entry in exec.layout.into_iter() {
        layout.entries[idx] = entry;
        idx += 1;
    }

    // allocate and insert the system region containing temporary paging, gdt and idt
    let mut temp_sys_region = manager.allocate::<ReadWrite>(BMVM_GUEST_TMP_SYSTEM_SIZE)?;
    // write layout
    temp_sys_region.write_offset(BMVM_MEM_LAYOUT_TABLE.as_u64() as usize, layout_region.as_ref())?;
    // write GDT
    temp_sys_region.write_offset(BMVM_TMP_GDT.as_u64() as usize, gdt().as_ref())?;
    // write LDT
    temp_sys_region.write_offset(BMVM_TMP_IDT.as_u64() as usize, idt().as_ref())?;
    // write paging
    temp_sys_region.write_offset(BMVM_TMP_PAGING.as_u64() as usize, paging().as_ref())?;

    Ok(())
}

fn paging() -> Vec<u8> {
    let mut paging = Vec::new();
    paging
}

fn idt() -> Vec<u8> {
    let mut idt = Vec::new();
    idt
}

fn gdt() -> Vec<u8> {
    let mut gdt = Vec::new();
    gdt.extend_from_slice(&gdt_entry(0, 0, 0, 0));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x9A, 0xA));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x92, 0xC));
    gdt
}

const fn gdt_entry(base: u64, limit: u64, access_byte: u8, flags: u8) -> [u8; 8] {
    [
        (limit & 0xFF) as u8,
        ((limit >> 8) & 0xFF) as u8,
        (base & 0xFF) as u8,
        ((base >> 8) & 0xFF) as u8,
        ((base >> 16) & 0xFF) as u8,
        access_byte,
        ((limit >> 16) & 0x0F) as u8 | (flags << 4),
        (base >> 24) as u8,
    ]
}

// TODO
fn estimate_sys_region(layout: &LayoutTable) -> LayoutTableEntry {
    // approx number of pages without the system section to better approx the system section size
    let (_, pdpt, pdt, pt) = estimate_paging_size_requirements(&layout.as_vec());
    let user_approx = pdpt + pdt + pt;
    // the user required space + IDT + GDT (in pages); The space for the page tables managing the sys region
    // is at the moment not included!
    let min_sys_size = user_approx as u64 + 1 + 1;
    LayoutTableEntry(min_sys_size)
}
