use crate::alloc::{Manager, ReadWrite};
use crate::elf::ExecBundle;
use crate::vm::utils::estimate_page_count;
use crate::{BMVM_GUEST_SYSTEM, BMVM_GUEST_TMP_SYSTEM_SIZE, BMVM_MIN_TEXT_SEGMENT};
use bmvm_common::mem::{
    Align, DefaultAlign, Flags, LayoutTableEntry, Page1GiB, Page4KiB, VirtAddr, align_ceil,
};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, BMVM_TMP_GDT, BMVM_TMP_IDT, BMVM_TMP_PAGING};
use kvm_ioctls::VmFd;
use std::num::NonZeroUsize;

// Values used for system region requirement estimation
// ------------------------------------------------------------------------------------------------
const PAGE_TABLE_SIZE: u64 = 0x1000;
const NUM_SYS_BUFFER_PAGES: u64 = 4;
const IDT_SIZE: u64 = 0x1000;
const GDT_SIZE: u64 = 0x1000;
const IDT_PAGE_REQUIRED: usize = (align_ceil(IDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;
const GDT_PAGE_REQUIRED: usize = (align_ceil(GDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;

// Temporary system structure
// ------------------------------------------------------------------------------------------------
const IDT_LOCATION: u64 = 0x0000;
const GDT_LOCATION: u64 = 0x1000;
const PAGING_LOCATION: u64 = 0x2000;

// Paging Entry Flags
// ------------------------------------------------------------------------------------------------
const PAGE_FLAG_PRESENT: u64 = 1;
const PAGE_FLAG_WRITE: u64 = 1 << 1;
const PAGE_FLAG_USER: u64 = 1 << 2;
const PAGE_FLAG_HUGE: u64 = 1 << 7;
const PAGE_FLAG_EXECUTABLE: u64 = 1 << 63;

/// Setting up a minimal environment containing paging structure, IDT and GDT to be able to enter
/// long mode and start with the actual structure setup by the guest.
fn setup_longmode(exec: &ExecBundle, manager: &Manager, vm: &VmFd) -> anyhow::Result<()> {
    // allocate a region for the temporary system strutures
    let size_tmp_sys = NonZeroUsize::new(BMVM_GUEST_TMP_SYSTEM_SIZE as usize).unwrap();
    let mut temp_sys_region = manager.allocate::<ReadWrite>(size_tmp_sys, vm)?;

    // estimate the system region
    let mut layout = exec.layout.clone();
    let layout_sys = estimate_sys_region(&layout)?;
    layout.insert(0, layout_sys);

    // write GDT
    temp_sys_region.write_offset(BMVM_TMP_GDT.as_usize(), gdt().as_ref())?;
    // write LDT
    temp_sys_region.write_offset(BMVM_TMP_IDT.as_usize(), idt().as_ref())?;
    // write paging
    for (idx, entry) in paging(&layout).iter() {
        let offset = idx * 8;
        temp_sys_region.write_offset(BMVM_TMP_PAGING.as_u64() as usize + offset, entry)?;
    }

    // write layout table
    for (i, entry) in layout.iter().enumerate() {
        let offset = i * size_of::<LayoutTableEntry>();
        temp_sys_region
            .write_offset(BMVM_MEM_LAYOUT_TABLE.as_usize() + offset, &entry.as_array())?;
    }

    Ok(())
}

/// Initializes a new Interrupt Descriptor Table (IDT).
/// Currently, this simply returns an empty vector, as no interrupt handler is registered.
fn idt() -> Vec<u8> {
    Vec::new()
}

/// Initialize a new Global Descriptor Table (GDT) valid in Long Mode.
fn gdt() -> Vec<u8> {
    let mut gdt = Vec::new();
    gdt.extend_from_slice(&gdt_entry(0, 0, 0, 0));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x9A, 0xA));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x92, 0xC));
    gdt
}

/// Constructs a new GDT entry
#[inline]
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

/// Create a basic paging structure which over-allocates the minimal required regions for the
/// program to execute properly:
/// * Code region including
/// * System region (be able to construct runtime paging structures etc)
///
/// # Returns
/// Vector of index and page entry.
/// The index is given as an index into the pageing structure. We are assuming a paging layout as follows:
/// [PML4][PDPT_1]...[PDPT_N][PD_1]...[PD_N][PT_1]...[PT_N]
/// The first entry in the PML4 results in an index of 0, the first entry 1, etc.
/// For PDPT_1 the index is in range [512, 1023], and so on.
fn paging(layout: &Vec<LayoutTableEntry>) -> Vec<(usize, [u8; 8])> {
    let mut output: Vec<(usize, [u8; 8])> = Vec::new();

    // entries for code and data segments
    // assuming the .text, .data, .rodata, .bss etc sections are all loaded continuously beginning
    // at well-defined address, we can very roughly create paging entries for the continuo region.
    let pdpt_1 = VirtAddr::new_truncate(BMVM_TMP_PAGING.as_u64() + PAGE_TABLE_SIZE);
    output.push((0, paging_entry(pdpt_1, false, false)));
    let mut non_sys_pages = cound_non_sys_pages(layout) as isize;
    let mut idx = 0;
    while non_sys_pages > 0 {
        let addr = BMVM_MIN_TEXT_SEGMENT + idx * Page1GiB::ALIGNMENT;
        output.push((
            (512 + idx) as usize,
            paging_entry(addr.as_virt_addr(), true, true),
        ));
        non_sys_pages -= (Page1GiB::ALIGNMENT / Page4KiB::ALIGNMENT) as isize;
        idx += 1;
    }

    // entries for the system region
    // a general 1GiB sized region will be mapped to be used for paging.
    let pdpt_2 = VirtAddr::new_truncate(BMVM_TMP_PAGING.as_u64() + PAGE_TABLE_SIZE * 2);
    let pml4_sys_idx = BMVM_GUEST_SYSTEM.as_virt_addr().p4_index();
    output.push((pml4_sys_idx.into(), paging_entry(pdpt_2, false, false)));
    output.push((
        1024,
        paging_entry(BMVM_GUEST_SYSTEM.as_virt_addr(), true, false),
    ));

    output
}

/// create a new paging entry
fn paging_entry(addr: VirtAddr, huge: bool, exec: bool) -> [u8; 8] {
    assert!(Page4KiB::is_aligned(addr.as_u64()));
    let mut value: u64 = PAGE_FLAG_PRESENT | PAGE_FLAG_WRITE | PAGE_FLAG_USER;
    value |= addr.as_u64() & 0xFFFF_FFFF_FFFF_F000;

    if huge {
        value |= PAGE_FLAG_HUGE
    }

    if !exec {
        value |= PAGE_FLAG_EXECUTABLE;
    }

    value.to_ne_bytes()
}

/// Based on the provided layout, the size of the system region will be estimated and
/// the resulting layout entry will be constructed.
fn estimate_sys_region(base: &Vec<LayoutTableEntry>) -> anyhow::Result<LayoutTableEntry> {
    if base.len() == 0 {
        return Err(anyhow::anyhow!("Empty layout"));
    }

    let mut estimate_has_converged = false;
    let mut layout = base.clone();

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

/// count all non-system sections and return the total number of required pages.
fn cound_non_sys_pages(layout: &Vec<LayoutTableEntry>) -> usize {
    let mut size = 0;
    layout.iter().for_each(|entry| {
        if !entry.flags().contains(Flags::SYSTEM) {
            size += entry.len() as usize
        }
    });

    size
}

mod test {
    #![allow(unused, dead_code)]

    use super::*;
    use bmvm_common::mem::{Page1GiB, Page2MiB, Page4KiB, PhysAddr};

    #[test]
    fn estimate_sys_region_single_region() {
        let base = vec![LayoutTableEntry::new(
            PhysAddr::new_truncate(0x20_0000),
            0x4_0000, // results in 1 GiB region
            Flags::PRESENT,
        )];

        assert_eq!((1, 1, 2, 0), estimate_page_count(&base));
        assert_eq!(9, estimate_sys_region(&base).unwrap().len());
    }

    #[test]
    fn estimate_sys_region_multiple_regions() {
        let base = vec![
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
                PhysAddr::new_truncate(
                    Page1GiB::ALIGNMENT * 511 + Page2MiB::ALIGNMENT + Page4KiB::ALIGNMENT * 4,
                ),
                0x40204, // results in 1GiB + 4MiB + 1 KiB region
                Flags::PRESENT,
            ),
        ];

        assert_eq!((1, 3, 5, 3), estimate_page_count(&base));
        assert_eq!(17, estimate_sys_region(&base).unwrap().len());
    }

    #[test]
    fn build_paging_structure() {
        let base = vec![
            LayoutTableEntry::new(
                PhysAddr::new_truncate(0x40_0000),
                0xf, // results in 1 GiB region
                Flags::PRESENT,
            ),
            LayoutTableEntry::new(
                PhysAddr::new_truncate(0x40_f000),
                1, // results in 4 KiB region
                Flags::PRESENT,
            ),
        ];

        assert_eq!(4, paging(&base).len());
    }
}
