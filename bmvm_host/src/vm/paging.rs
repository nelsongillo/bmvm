use crate::alloc::{Allocator, ReadWrite, Region};
use bmvm_common::mem::{
    Align, AlignedNonZeroUsize, Flags, LayoutTableEntry, Page1GiB, Page2MiB, Page4KiB, PhysAddr,
    VirtAddr, aligned_and_fits,
};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::num::NonZeroUsize;
use std::slice;

const PAGE_FLAG_PRESENT: u64 = 1;
const PAGE_FLAG_WRITE: u64 = 1 << 1;
const PAGE_FLAG_USER: u64 = 1 << 2;
const PAGE_FLAG_HUGE: u64 = 1 << 7;
const PAGE_FLAG_NOT_EXECUTABLE: u64 = 1 << 63;

// 52-bit physical address mask (bits 51:12) in entries
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// ---------- Inputs ----------

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Empty Module")]
    EmptyModule,
    #[error("Error during region allocation: {0}")]
    AllocError(#[from] crate::alloc::Error),
    #[error("Unknown region index {0}")]
    UnknownRegionIndex(usize),
    #[error("Expected region for address {0:x}, but got none")]
    NoRegionForAddr(PhysAddr),
    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(usize),
    #[error("Overlapping page: {0:x}")]
    Overlapping(PhysAddr),
}

#[derive(Clone, Copy, Hash)]
pub struct Page {
    pub region: usize, // index in region structure
    pub offset: usize, // offset in the region (multiple of 0x1000)
}

pub struct PagingArena<'a> {
    allocator: &'a Allocator,
    regions: Vec<Region<ReadWrite>>,
    pages: HashMap<PhysAddr, Page>,
    current: usize,
    offset: usize,
    remaining: usize,
    on_demand: usize,
    next_addr: PhysAddr,
}

impl<'a> PagingArena<'a> {
    pub fn new(
        allocator: &'a Allocator,
        pml4: PhysAddr,
        initial: NonZeroUsize,
        on_demand: NonZeroUsize,
    ) -> Result<Self> {
        let capactity =
            AlignedNonZeroUsize::new_aligned(initial.get() * Page4KiB::ALIGNMENT as usize).unwrap();
        let base = allocator
            .alloc_accessible::<ReadWrite>(capactity)?
            .set_guest_addr(pml4);
        let regions = vec![base];
        let mut pages = HashMap::new();
        pages.insert(
            pml4,
            Page {
                region: 0,
                offset: 0,
            },
        );

        Ok(Self {
            allocator,
            regions,
            pages,
            current: 0,
            offset: 0,
            remaining: initial.get(),
            on_demand: on_demand.get(),
            next_addr: pml4 + Page4KiB::ALIGNMENT,
        })
    }

    /// Try fetching the table at a given address
    fn table_at(&self, addr: PhysAddr) -> Option<&mut [u8]> {
        // If we have a page for this address, return it
        if let Some(page) = self.pages.get(&addr) {
            let region = self.regions.get(page.region)?;
            let from = page.offset * Page4KiB::ALIGNMENT as usize;
            let to = from + Page4KiB::ALIGNMENT as usize;

            let ptr = region.as_ptr().cast_mut();
            let slice = unsafe { slice::from_raw_parts_mut(ptr, region.capacity().get()) };
            let p = &mut slice[from..to];
            return Some(p);
        };

        None
    }

    /// Create a new child table for the parent at an index with the given flags.
    ///
    /// # Parameter
    /// * parent: The address of the parent table (here the new entry for the child will be written)
    /// * idx: the index to write the child entry to
    /// * flags: Write/Execute permission for the child
    ///
    /// # Returns
    /// Result containing the physical address of the newly created child table.
    fn child(&mut self, parent: PhysAddr, idx: usize, flags: Flags) -> Result<PhysAddr> {
        // address to create the child table at
        let addr = self.next_addr;
        self.next_addr += Page4KiB::ALIGNMENT;

        // if no more pages are available, allocate a new region
        if self.remaining == 0 {
            let on_demand =
                AlignedNonZeroUsize::new_aligned(self.on_demand * Page4KiB::ALIGNMENT as usize)
                    .unwrap();
            let region = self
                .allocator
                .alloc_accessible::<ReadWrite>(on_demand)?
                .set_guest_addr(addr);
            self.regions.push(region);
            self.current = self.regions.len() - 1;
            self.offset = 0;
            self.remaining = self.on_demand;
        }

        // update the arena allocator
        self.remaining -= 1;
        self.offset += 1;
        self.pages.insert(
            addr,
            Page {
                region: self.current,
                offset: self.offset,
            },
        );

        // write the entry to the parent table
        let parent_table = self
            .table_at(parent)
            .ok_or(Error::NoRegionForAddr(parent))?;
        let entry = PageEntry::new(addr.as_u64(), false, flags);
        write_at(parent_table, idx, entry)?;

        Ok(addr)
    }

    fn into_regions(self) -> Vec<Region<ReadWrite>> {
        self.regions
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
struct PageEntry(u64);

impl PageEntry {
    fn new(addr: u64, huge: bool, flags: Flags) -> Self {
        assert!(Page4KiB::is_aligned(addr));
        let mut entry: u64 = PAGE_FLAG_PRESENT;
        entry |= addr & ADDR_MASK;

        if huge {
            entry |= PAGE_FLAG_HUGE;
        }

        if !flags.is_code() {
            entry |= PAGE_FLAG_NOT_EXECUTABLE;
        }

        if flags.is_write() {
            entry |= PAGE_FLAG_WRITE;
        }

        Self(entry)
    }

    const fn set_write(&mut self, write: bool) {
        if write {
            self.0 |= PAGE_FLAG_WRITE;
        } else {
            self.0 &= !PAGE_FLAG_WRITE;
        }
    }

    const fn set_exec(&mut self, exec: bool) {
        if exec {
            self.0 &= !PAGE_FLAG_NOT_EXECUTABLE;
        } else {
            self.0 |= PAGE_FLAG_NOT_EXECUTABLE;
        }
    }

    fn set_addr(&mut self, addr: PhysAddr) {
        assert!(Page4KiB::is_aligned(addr.as_u64()));
        self.0 &= !ADDR_MASK;
        self.0 |= addr.as_u64() & ADDR_MASK;
    }

    const fn present(&self) -> bool {
        self.0 & PAGE_FLAG_PRESENT != 0
    }

    const fn huge(&self) -> bool {
        self.0 & PAGE_FLAG_HUGE != 0
    }

    const fn write(&self) -> bool {
        self.0 & PAGE_FLAG_WRITE != 0
    }

    const fn exec(&self) -> bool {
        self.0 & PAGE_FLAG_NOT_EXECUTABLE == 0
    }

    fn addr(&self) -> u64 {
        self.0 & ADDR_MASK
    }

    const fn to_ne_bytes(self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl From<u64> for PageEntry {
    fn from(entry: u64) -> Self {
        Self(entry)
    }
}

impl Display for PageEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Present: {}, Huge: {}, Write: {}, Exec: {}, Addr: {:x}",
            self.present(),
            self.huge(),
            self.write(),
            self.exec(),
            self.addr()
        )
    }
}

/// Build the guest paging structure
pub(super) fn setup(
    allocator: &Allocator,
    entries: &[LayoutTableEntry],
    pml4: PhysAddr,
    initial: NonZeroUsize,
    on_demand: NonZeroUsize,
) -> Result<Vec<Region<ReadWrite>>> {
    let mut arena = PagingArena::new(allocator, pml4, initial, on_demand)?;

    for layout_entry in entries.iter() {
        let mut addr = layout_entry.addr().as_virt_addr();
        let end = addr + layout_entry.size() - 1;
        let flags = layout_entry.flags();
        log::info!(
            "Mapping {:x} - {:x} with flags {:?}",
            addr.as_u64(),
            end.as_u64(),
            flags
        );
        while addr < end {
            match () {
                _ if aligned_and_fits::<Page1GiB>(addr.as_u64(), end.as_u64()) => {
                    log::debug!("Mapping 1GiB page at {:x}", addr.as_u64());
                    let pdpt = write_idx(&mut arena, addr, pml4, addr.p4_index(), flags)?;

                    // Handle leaf entry
                    let table = arena.table_at(pdpt).ok_or(Error::NoRegionForAddr(pdpt))?;
                    let entry = PageEntry::new(addr.as_u64(), true, flags);
                    write_at(table, addr.p3_index(), entry)?;
                    addr += Page1GiB::ALIGNMENT;
                }
                _ if aligned_and_fits::<Page2MiB>(addr.as_u64(), end.as_u64()) => {
                    log::debug!("Mapping 2MiB page at {:x}", addr.as_u64());
                    let pdpt = write_idx(&mut arena, addr, pml4, addr.p4_index(), flags)?;
                    let pd = write_idx(&mut arena, addr, pdpt, addr.p3_index(), flags)?;

                    // Handle leaf entry
                    let table = arena.table_at(pd).ok_or(Error::NoRegionForAddr(pd))?;
                    let entry = PageEntry::new(addr.as_u64(), true, flags);
                    write_at(table, addr.p2_index(), entry)?;
                    addr += Page2MiB::ALIGNMENT;
                }
                _ => {
                    log::debug!("Mapping 4KiB page at {:x}", addr.as_u64());
                    let pdpt = write_idx(&mut arena, addr, pml4, addr.p4_index(), flags)?;
                    let pd = write_idx(&mut arena, addr, pdpt, addr.p3_index(), flags)?;
                    let pt = write_idx(&mut arena, addr, pd, addr.p2_index(), flags)?;

                    // Handle leaf entry
                    let table = arena.table_at(pt).ok_or(Error::NoRegionForAddr(pt))?;
                    let entry = PageEntry::new(addr.as_u64(), false, flags);
                    write_at(table, addr.p1_index(), entry)?;
                    addr += Page4KiB::ALIGNMENT;
                }
            }
        }
    }

    Ok(arena.into_regions())
}

#[inline]
fn get_at(table: &[u8], idx: usize) -> Result<PageEntry> {
    let offset = idx * size_of::<u64>();
    if offset + size_of::<u64>() > table.len() {
        return Err(Error::IndexOutOfBounds(idx));
    }
    let entry = &table[offset..offset + size_of::<u64>()];
    Ok(PageEntry::from(u64::from_ne_bytes(
        entry.try_into().unwrap(),
    )))
}

/// Write a page entry to the table at the given index.
#[inline]
fn write_at(table: &mut [u8], idx: usize, entry: PageEntry) -> Result<()> {
    log::debug!("Index: {idx} - Entry {entry}");

    let offset = idx * size_of::<PageEntry>();
    if offset + size_of::<u64>() > table.len() {
        return Err(Error::IndexOutOfBounds(idx));
    }
    table[offset..offset + size_of::<u64>()].copy_from_slice(entry.to_ne_bytes().as_slice());
    Ok(())
}

/// Write a page entry to the table at the given index which points to a child table. If the entry
/// is not present, the child table should be initialized in addition to writing the parent table
/// entry. If the Entry was previously present, and the flags do not grant more permissions than
/// previously set, this function is a no-op regarding parent page modification.
fn write_idx(
    arena: &mut PagingArena,
    addr_target: VirtAddr,
    addr_table: PhysAddr,
    idx: usize,
    flags: Flags,
) -> Result<PhysAddr> {
    let table = arena
        .table_at(addr_table)
        .ok_or(Error::NoRegionForAddr(addr_table))?;

    let mut entry = get_at(table, idx)?;

    if entry.present() && entry.huge() {
        return Err(Error::Overlapping(PhysAddr::from(addr_target)));
    }

    if !entry.present() {
        let next_table = arena.child(addr_table, idx, flags)?;
        return Ok(next_table);
    }

    let mut modified = false;
    // Update the permissions to the most permissive.
    if !entry.exec() && flags.is_code() {
        entry.set_exec(true);
        log::debug!("Modified: EXEC");
        modified = true;
    }
    // Update the permissions to the most permissive.
    if !entry.write() && flags.is_write() {
        entry.set_write(true);
        log::debug!("Modified: WRITE");
        modified = true;
    }

    if modified {
        write_at(table, idx, entry)?;
    }

    Ok(PhysAddr::new(entry.addr()))
}
