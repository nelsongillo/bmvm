use crate::{GUEST_SYSTEM_ADDR, MIN_TEXT_SEGMENT};
use bmvm_common::BMVM_TMP_PAGING;
use bmvm_common::mem::{
    Align, DefaultAlign, Flags, LayoutTableEntry, Page1GiB, Page2MiB, Page4KiB, VirtAddr,
    align_ceil, aligned_and_fits,
};
use std::collections::HashSet;

// Values used for system region requirement estimation
// ------------------------------------------------------------------------------------------------
const PAGE_TABLE_SIZE: u64 = 0x1000;
pub(super) const IDT_SIZE: u64 = 0x1000;
pub(super) const GDT_SIZE: u64 = 0x1000;
const IDT_PAGE_REQUIRED: usize = (align_ceil(IDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;
const GDT_PAGE_REQUIRED: usize = (align_ceil(GDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;

// Paging Entry Flags
// ------------------------------------------------------------------------------------------------
const PAGE_FLAG_PRESENT: u64 = 1;
const PAGE_FLAG_WRITE: u64 = 1 << 1;
const PAGE_FLAG_USER: u64 = 1 << 2;
const PAGE_FLAG_HUGE: u64 = 1 << 7;
const PAGE_FLAG_EXECUTABLE: u64 = 1 << 63;

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Empty Module")]
    EmptyModule,
}

/// Initializes a new Interrupt Descriptor Table (IDT).
/// Currently, this simply returns an empty vector, as no interrupt handler is registered.
pub(crate) fn idt() -> Vec<u8> {
    Vec::new()
}

/// Initialize a new Global Descriptor Table (GDT) valid in Long Mode.
pub(crate) fn gdt() -> Vec<u8> {
    let mut gdt = Vec::new();
    gdt.extend_from_slice(&gdt_entry(0, 0, 0, 0));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x9A, 0xA));
    gdt.extend_from_slice(&gdt_entry(0, 0xFFFFF, 0x92, 0xC));
    gdt
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
pub(crate) fn paging(layout: &Vec<LayoutTableEntry>) -> Vec<(usize, [u8; 8])> {
    let mut output: Vec<(usize, [u8; 8])> = Vec::new();

    // entries for code and data segments
    // assuming the .text, .data, .rodata, .bss etc sections are all loaded continuously beginning
    // at well-defined address, we can very roughly create paging entries for the continuo region.
    let pdpt_1 = VirtAddr::new_truncate(BMVM_TMP_PAGING.as_u64() + PAGE_TABLE_SIZE);
    output.push((0, paging_entry(pdpt_1, false, false)));
    let mut non_sys_pages = cound_non_sys_pages(layout) as isize;
    let mut idx = 0;
    while non_sys_pages > 0 {
        let addr = MIN_TEXT_SEGMENT + idx * Page1GiB::ALIGNMENT;
        output.push((
            (512 + idx) as usize,
            paging_entry(VirtAddr::new_truncate(addr), true, true),
        ));
        non_sys_pages -= (Page1GiB::ALIGNMENT / Page4KiB::ALIGNMENT) as isize;
        idx += 1;
    }

    // entries for the system region
    // a general 1GiB sized region will be mapped to be used for paging.
    let sys_addr = GUEST_SYSTEM_ADDR().as_virt_addr();
    let pdpt_2 = VirtAddr::new_truncate(BMVM_TMP_PAGING.as_u64() + PAGE_TABLE_SIZE * 2);
    let pml4_sys_idx = sys_addr.p4_index();
    output.push((pml4_sys_idx.into(), paging_entry(pdpt_2, false, false)));
    output.push((1024, paging_entry(sys_addr, true, false)));

    output
}

/// Based on the provided layout, the size of the system region will be estimated and
/// the resulting layout entry will be constructed.
pub(super) fn estimate_sys_region(base: &Vec<LayoutTableEntry>) -> Result<LayoutTableEntry> {
    if base.len() == 0 {
        return Err(Error::EmptyModule);
    }

    let mut estimate_has_converged = false;
    let mut layout = base.clone();

    // approximate only user space requirements and construct base system requirements
    let (pml4, pdpt, pdt, pt) = estimate_page_count(&layout);
    let mut estimate = pml4 + pdpt + pdt + pt + IDT_PAGE_REQUIRED + GDT_PAGE_REQUIRED;
    loop {
        let sys = LayoutTableEntry::new(GUEST_SYSTEM_ADDR(), estimate as u32, Flags::empty());
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
        GUEST_SYSTEM_ADDR(),
        estimate as u32,
        Flags::PRESENT,
    ))
}

/// This function tries to estimate the number of pages required to identity map all the regions
/// in the given layout.
///
/// # Returns
///
/// - The number of pages required per level: (PML4, PDPT, PD, PT)
pub(crate) fn estimate_page_count(regions: &Vec<LayoutTableEntry>) -> (usize, usize, usize, usize) {
    if regions.is_empty() {
        return (0, 0, 0, 0);
    }

    let mut pml4_indices = HashSet::new();
    let mut pdpt_indices = HashSet::new();
    let mut pd_indices = HashSet::new();

    for region in regions.iter() {
        let mut addr = region.addr().as_virt_addr();
        let end = VirtAddr::new_truncate((addr + region.size()).as_u64());

        while addr < end {
            match addr {
                _ if aligned_and_fits::<Page1GiB>(addr.as_u64(), end.as_u64()) => {
                    let pml4 = addr.p4_index();
                    pml4_indices.insert(pml4);
                    addr += Page1GiB::ALIGNMENT;
                }
                _ if aligned_and_fits::<Page2MiB>(addr.as_u64(), end.as_u64()) => {
                    let pml4 = addr.p4_index();
                    let pdpt = addr.p3_index();
                    pml4_indices.insert(pml4);
                    pdpt_indices.insert((pml4, pdpt));
                    addr += Page2MiB::ALIGNMENT;
                }
                _ => {
                    let pml4 = addr.p4_index();
                    let pdpt = addr.p3_index();
                    let pd = addr.p2_index();
                    pml4_indices.insert(pml4);
                    pdpt_indices.insert((pml4, pdpt));
                    pd_indices.insert((pml4, pdpt, pd));
                    addr += Page4KiB::ALIGNMENT;
                }
            }
        }
    }

    (1, pml4_indices.len(), pdpt_indices.len(), pd_indices.len())
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
    fn empty() {
        let regions = Vec::new();
        let result = estimate_page_count(&regions);
        assert_eq!((0, 0, 0, 0), result);
    }

    #[test]
    fn single_4k_page() {
        let mut regions = Vec::new();
        regions.push(create_entry(0x1000, 0x1000)); // One 4K page at 0x1000

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 1, 1), result);
    }

    #[test]
    fn multiple_4k_pages_same_pd() {
        let mut regions = Vec::new();
        // 5 pages that all map to the same PD entry (same top 30 bits)
        for i in 0..5 {
            regions.push(create_entry(0x1000 + i * 0x1000, 0x1000));
        }

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 1, 1), result);
    }

    #[test]
    fn single_2mb_page_aligned() {
        let mut regions = Vec::new();
        // A single 2MB-aligned region of 2MB size
        regions.push(create_entry(0x200000, 0x200000));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 1, 0), result);
    }

    #[test]
    fn single_1gb_page_aligned() {
        let mut regions = Vec::new();
        // A single 1GB-aligned region of 1GB size
        regions.push(create_entry(0x40000000, 0x40000000));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 0, 0), result);
    }

    #[test]
    fn misaligned_region() {
        let mut regions = Vec::new();
        // A region not aligned to 2MB or 1GB
        regions.push(create_entry(0x201000, 0x1000));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 1, 1), result);
    }

    #[test]
    fn region_crossing_boundaries() {
        let mut regions = Vec::new();
        // A region crossing PD boundaries (0x3FFFFF crosses into next 2MB page)
        regions.push(create_entry(0x1FF000, 0x203000));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 1, 3), result);
    }

    #[test]
    fn crossing_pd_boundary() {
        let mut regions = Vec::new();
        // Region crossing PD boundary (0x3FF00000 crosses into next 1GB page)
        let start = 0x3FE00000;
        let size = 0x400000; // crosses the 1GB boundary
        regions.push(create_entry(start, size));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 2, 0), result);
    }

    #[test]
    fn crossing_pdpt_boundary() {
        let mut regions = Vec::new();
        // Region that crosses a PDPT boundary (0x3FFFFFF000 crosses PML4 index)
        let start = 0x3FFFFFF000;
        let size = 0x3000; // small size crossing PML4 boundary
        regions.push(create_entry(start, size));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 2, 2), result);
    }

    #[test]
    fn mixed_page_sizes() {
        let mut regions = Vec::new();

        // A 1GB-aligned region of 1GB size
        regions.push(create_entry(0x40000000, 0x40000000));

        // A 2MB-aligned region of 2MB size
        regions.push(create_entry(0x80200000, 0x200000));

        // A 4KB page on a different PD
        regions.push(create_entry(0xC0001000, 0x1000));

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 2, 1), result);
    }

    #[test]
    fn multiple_regions_different_addresses() {
        let mut regions = Vec::new();

        // Different regions at different memory locations
        regions.push(create_entry(0x1000, 0x1000)); // 4KB at 0x1000
        regions.push(create_entry(0x40000000, 0x40000000)); // 1GB at 1GB
        regions.push(create_entry(0x80000000, 0x200000)); // 2MB at 2GB

        let result = estimate_page_count(&regions);
        assert_eq!((1, 1, 2, 1), result);
    }

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

    // shortcut to creating a present entry with size
    fn create_entry(addr: u64, size_bytes: u64) -> LayoutTableEntry {
        LayoutTableEntry::new(
            PhysAddr::new_truncate(addr),
            (size_bytes / DefaultAlign::ALIGNMENT) as u32,
            Flags::PRESENT,
        )
    }
}
