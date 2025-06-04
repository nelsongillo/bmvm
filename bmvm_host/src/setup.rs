use crate::alloc::{Manager, ReadWrite};
use crate::elf::ExecBundle;
use crate::utils::estimate_page_count;
use crate::{BMVM_GUEST_SYSTEM, BMVM_GUEST_TMP_SYSTEM_SIZE};
use bmvm_common::interprete::Interpret;
use bmvm_common::mem::{Align, DefaultAlign, Flags, LayoutTable, LayoutTableEntry, align_ceil};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, BMVM_TMP_GDT, BMVM_TMP_IDT, BMVM_TMP_PAGING};

const NUM_SYS_BUFFER_PAGES: u64 = 4;
const PAGE_TABLE_SIZE: u64 = 0x1000;
const IDT_SIZE: u64 = 0x1000;
const IDT_PAGE_REQUIRED: usize = (align_ceil(IDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;
const GDT_SIZE: u64 = 0x1000;
const GDT_PAGE_REQUIRED: usize = (align_ceil(GDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;

const IDT_LOCATION: u64 = 0x0000;
const GDT_LOCATION: u64 = 0x1000;
const PAGING_LOCATION: u64 = 0x2000;

fn setup_longmode(exec: &ExecBundle, manager: impl Manager) -> anyhow::Result<()> {
    let mut layout_region = manager.allocate::<ReadWrite>(size_of::<LayoutTable>() as u64)?;
    let layout = LayoutTable::from_mut_bytes(layout_region.as_mut())?;

    layout.entries[0] = estimate_sys_region(&exec.layout)?;
    let mut idx = 1;
    for entry in exec.layout.into_iter() {
        layout.entries[idx] = entry;
        idx += 1;
    }

    // allocate and insert the system region containing temporary paging, gdt and idt
    let mut temp_sys_region = manager.allocate::<ReadWrite>(BMVM_GUEST_TMP_SYSTEM_SIZE)?;
    // write layout
    temp_sys_region.write_offset(
        BMVM_MEM_LAYOUT_TABLE.as_u64() as usize,
        layout_region.as_ref(),
    )?;
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

fn estimate_sys_region(base: &LayoutTable) -> anyhow::Result<LayoutTableEntry> {
    if base.len_present() == 0 {
        return Err(anyhow::anyhow!("Empty layout"));
    }

    let mut estimate_has_converged = false;
    let mut layout: Vec<LayoutTableEntry> = base.as_vec_present().clone();

    // approximate only user space requirements and construct base system requirements
    let (pml4, pdpt, pdt, pt) = estimate_page_count(&layout);
    let mut estimate = pml4 + pdpt + pdt + pt + IDT_PAGE_REQUIRED + GDT_PAGE_REQUIRED;
    loop {
        let sys = LayoutTableEntry::new(BMVM_GUEST_SYSTEM, estimate as u32, Flags::empty());
        layout.push(sys);

        // estimate has converged to stable value -> break and return final estimate
        if estimate_has_converged {
            break;
        }

        // recalculate the space requirement
        let (pml4, pdpt, pdt, pt) = estimate_page_count(&layout);
        let current = pml4 + pdpt + pdt + pt + IDT_PAGE_REQUIRED + GDT_PAGE_REQUIRED;

        // update converging flag and estimate
        estimate_has_converged = estimate == current;
        estimate = current;
        _ = layout.pop(); // remove the temporary system region
    }
    // the user required space + IDT + GDT (in pages); The space for the page tables managing the sys region
    Ok(LayoutTableEntry::new(
        BMVM_GUEST_SYSTEM,
        estimate as u32,
        Flags::PRESENT,
    ))
}

mod test {
    #![allow(unused, dead_code)]

    use super::*;
    use bmvm_common::mem::{Page1GiB, Page2MiB, Page4KiB, PhysAddr};


    #[test]
    fn estimate_sys_region_single_region() {
        let base = LayoutTable::from_vec(&vec![LayoutTableEntry::new(
            PhysAddr::new_truncate(0x20_0000),
            0x4_0000, // results in 1 GiB region
            Flags::PRESENT,
        )])
        .unwrap();

        assert_eq!((1, 1, 2, 0), estimate_page_count(&base.as_vec_present()));
        assert_eq!(9, estimate_sys_region(&base).unwrap().len());
    }

    #[test]
    fn estimate_sys_region_multiple_regions() {
        let base = LayoutTable::from_vec(&vec![
            LayoutTableEntry::new(
                PhysAddr::new_truncate(0x20_0000),
                0x4_0000, // results in 1 GiB region
                Flags::PRESENT,
            ),
            LayoutTableEntry::new(
                PhysAddr::new_truncate(Page1GiB::ALIGNMENT * 1024),
                1, // results in 4 KiB region
                Flags::PRESENT,
            ),
            LayoutTableEntry::new(
                PhysAddr::new_truncate(Page1GiB::ALIGNMENT * 511 + Page2MiB::ALIGNMENT + Page4KiB::ALIGNMENT * 4),
                0x40204, // results in 1GiB + 4MiB + 1 KiB region
                Flags::PRESENT,
            ),
        ])
        .unwrap();

        assert_eq!((1, 3, 5, 3), estimate_page_count(&base.as_vec_present()));
        assert_eq!(17, estimate_sys_region(&base).unwrap().len());
    }
}
