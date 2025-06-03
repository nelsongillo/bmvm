use bmvm_common::mem::{
    Align, LayoutTable, LayoutTableEntry, Page1GiB, Page2MiB, Page4KiB, PhysAddr, VirtAddr,
    aligned_and_fits,
};
use std::collections::HashSet;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bmvm_common::mem::{DefaultAlign, Flags, LayoutTableEntry, PhysAddr};

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

    // shortcut to creating a present entry with size
    fn create_entry(addr: u64, size_bytes: u64) -> LayoutTableEntry {
        LayoutTableEntry::new(
            PhysAddr::new_truncate(addr),
            (size_bytes / DefaultAlign::ALIGNMENT) as u32,
            Flags::PRESENT,
        )
    }
}
