
use bmvm_common::mem::{aligned_and_fits, Align, LayoutTable, LayoutTableEntry, Page1GiB, Page2MiB, Page4KiB, PhysAddr, VirtAddr};
use std::collections::HashSet;

/// This function tries to estimate the number of pages required to identity map all the regions
/// in the given layout.
///
/// # Returns
///
/// - The number of pages required per level: (PML4, PDPT, PD, PT)
pub(crate) fn estimate_paging_size_requirements(regions: &Vec<LayoutTableEntry>) -> (usize, usize, usize, usize) {
    let mut pml4_indices = HashSet::new();
    let mut pdpt_indices = HashSet::new();
    let mut pd_indices = HashSet::new();
    let mut pt_indices = HashSet::new();

    for region in regions.iter() {
        let mut addr = region.addr().as_virt_addr();
        let end = VirtAddr::new_truncate((addr + region.size()).as_u64());

        while addr < end {
            match addr {
                _ if aligned_and_fits::<Page1GiB>(addr.as_u64(), end.as_u64()) => {
                    let pml4 = addr.p1_index();
                    let pdpt = addr.p2_index();
                    pml4_indices.insert(pml4);
                    pdpt_indices.insert((pml4, pdpt));
                    addr += Page1GiB::ALIGNMENT;
                }
                _ if aligned_and_fits::<Page2MiB>(addr.as_u64(), end.as_u64())=> {
                    let pml4 = addr.p1_index();
                    let pdpt = addr.p2_index();
                    let pd = addr.p2_index();
                    pml4_indices.insert(pml4);
                    pdpt_indices.insert((pml4, pdpt));
                    pd_indices.insert((pml4, pdpt, pd));
                    addr += Page2MiB::ALIGNMENT;
                }
                _ => {
                    let pml4 = addr.p1_index();
                    let pdpt = addr.p2_index();
                    let pd = addr.p2_index();
                    let pt = addr.p4_index();
                    pml4_indices.insert(pml4);
                    pdpt_indices.insert((pml4, pdpt));
                    pd_indices.insert((pml4, pdpt, pd));
                    pt_indices.insert((pml4, pdpt, pd, pt));
                    addr += Page4KiB    ;
                }
            }
        }
    }

    (
        pml4_indices.len(),
        pdpt_indices.len(),
        pd_indices.len(),
        pt_indices.len(),
    )
}
