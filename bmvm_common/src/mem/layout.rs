use crate::interprete::{Interpret, Zero};
use crate::mem::{Align, AlignedNonZeroUsize, Arena, DefaultAlign, PhysAddr};
use bitflags::bitflags;
#[cfg(feature = "vmi-consume")]
use core::fmt::{Display, Formatter};
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use x86_64::structures::paging::PageTableFlags;

pub const MAX_REGION_SIZE: u64 = u16::MAX as u64 * DefaultAlign::ALIGNMENT;

#[repr(C)]
pub struct LayoutTable {
    pub entries: [LayoutTableEntry; 512],
}

impl LayoutTable {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "vmi-consume")]
    pub fn len_present(&self) -> usize {
        self.as_vec_present().len()
    }

    #[cfg(feature = "vmi-consume")]
    pub fn from_vec(vec: &[LayoutTableEntry]) -> Result<LayoutTable, &'static str> {
        if vec.len() > 512 {
            return Err("layout table cannot contain more than 512 entries");
        }
        let mut l = LayoutTable::new();
        for (idx, e) in vec.iter().enumerate() {
            l.entries[idx] = *e;
        }
        Ok(l)
    }

    #[cfg(feature = "vmi-consume")]
    pub fn as_vec_present(&self) -> Vec<LayoutTableEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_present())
            .copied()
            .collect::<Vec<LayoutTableEntry>>()
    }

    pub fn find_intersect(&self, flag: Flags) -> Option<(usize, LayoutTableEntry)> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, e)| e.flags().intersects(flag))
            .map(|(i, e)| (i, *e))
    }
}

pub struct LayoutTableIter<'a> {
    iter: &'a LayoutTable,
    idx: usize,
}

impl Default for LayoutTable {
    fn default() -> Self {
        Self {
            entries: [LayoutTableEntry::empty(); 512],
        }
    }
}

impl Zero for LayoutTable {
    fn zero(&mut self) {
        self.entries.fill(LayoutTableEntry(0));
    }
}

impl Interpret for LayoutTable {}

impl Iterator for LayoutTableIter<'_> {
    type Item = LayoutTableEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.iter.entries[self.idx];
        if curr.is_present() {
            self.idx += 1;
            return Some(curr);
        };

        None
    }
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct Flags: u8 {
        /// Indicates the entrys presence.
        const PRESENT = 0b0000_0001;
        ///  0 -> User; 1 -> System structures
        const USER = 0b0000_0000;
        const SYSTEM = 0b0000_0010;
        const STACK = 0b0000_0100;
        /// 0 -> Data; 1 -> Code/Executable
        const CODE = 0b0000_1000;
        const DATA = 0b0000_0000;
        /// 0 -> Read; 1 -> Write
        const READ = 0b0000_0000;
        const WRITE = 0b0001_0000;
        const SHARED_FOREIGN = 0b0010_0000;
        const SHARED_OWNED = 0b0011_0000;
    }
}

impl<'a> IntoIterator for &'a LayoutTable {
    type Item = LayoutTableEntry;
    type IntoIter = LayoutTableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        LayoutTableIter { iter: self, idx: 0 }
    }
}

impl Flags {
    /// Converts the flags to their `PageTableFlags` representation.
    /// Note: The present flag will not be set by this method.
    pub fn to_page_table_flags(self) -> PageTableFlags {
        let mut pt_flags = PageTableFlags::PRESENT;

        // If the entry indicates that the page is a data page, check writeable
        if !self.contains(Flags::CODE) {
            pt_flags |= PageTableFlags::NO_EXECUTE;
            if self.contains(Flags::WRITE) {
                pt_flags |= PageTableFlags::WRITABLE;
            }
        }

        pt_flags
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct LayoutTableEntry(u64);

/// The layout table entry is 64 bits wide, so we can use the first 63 bits for the size
/// 0: Present bit - if set, the entry is valid
/// 1: 0 -> User; 1 -> System structures
/// 2: Stack
/// 3: 0 -> Data; 1 -> Code/Executable
/// 4-5:
///     If Section is Data:
///         00 -> Read
///         01 -> Write
///         10 -> Shared Foreign
///         11 -> Shared Owned
///     Else: Unsued
/// 8-24: multiplicator of pages
/// 24-63: starting address
impl LayoutTableEntry {
    const MASK_RETRIEVE_FLAGS: u64 = 0x0000_0000_0000_00ff;
    const MASK_RETRIEVE_SIZE: u64 = 0x0000_0000_0fff_ff00;
    const MASK_RETRIEVE_ADDR: u64 = 0xffff_ffff_f000_0000;

    /// Creates a new LayoutTableEntry with the given parameters.
    ///
    /// # Parameters
    /// * `addr` - The physical address, where the region should start. Must be a valid paged aligned physical address.
    /// * `size` - Indicates the number of pages the region is spanning. Limited to 20bit, resulting in maximal 4GiB big regions.
    /// * `flags` - The flags mark the entry for a specific use-case, which will result in the equivalent paging flags.
    pub fn new(addr: PhysAddr, size: u32, flags: Flags) -> Self {
        assert!(
            DefaultAlign::is_aligned(addr.as_u64()),
            "addr must be page aligned"
        );
        assert!(
            Self::is_valid_size(size),
            "size must not be zero and exceed 20 bits"
        );
        let mut value = flags.bits() as u64;

        // set the size
        value |= (size as u64) << 8;
        // set the address
        value |= (addr.as_u64() & 0xFFFF_FFFF_FFFF) << 16;

        LayoutTableEntry(value)
    }

    /// Creates a new empty layoutTableEntry
    pub const fn empty() -> Self {
        LayoutTableEntry(0)
    }

    pub fn set_present(&mut self, present: bool) -> &mut Self {
        let mut flags = self.flags();
        flags.set(Flags::PRESENT, present);
        self.set_flags(flags)
    }

    pub const fn set_flags(&mut self, flags: Flags) -> &mut Self {
        self.0 &= !Self::MASK_RETRIEVE_FLAGS;
        self.0 |= flags.bits() as u64;
        self
    }

    pub const fn set_len(&mut self, size: u32) -> &mut Self {
        self.0 &= !Self::MASK_RETRIEVE_SIZE;
        self.0 |= (size as u64) << 8;
        self
    }

    pub fn set_addr(&mut self, addr: PhysAddr) -> &mut Self {
        assert!(
            DefaultAlign::is_aligned(addr.as_u64()),
            "addr must be page aligned"
        );
        self.0 &= !Self::MASK_RETRIEVE_ADDR;
        self.0 |= (addr.as_u64()) << 16;
        self
    }

    /// Checks if the entry is present
    pub const fn is_present(&self) -> bool {
        self.flags().contains(Flags::PRESENT)
    }

    pub const fn flags(&self) -> Flags {
        let f = self.0 & Self::MASK_RETRIEVE_FLAGS;
        Flags::from_bits_truncate(f as u8)
    }

    /// Gets the number of pages included in the entry
    pub const fn len(&self) -> u32 {
        ((self.0 & Self::MASK_RETRIEVE_SIZE) >> 8) as u32
    }

    /// Returns the size of the entry in bytes. The size is page-aligned.
    pub const fn size(&self) -> u64 {
        self.len() as u64 * DefaultAlign::ALIGNMENT
    }

    /// Returns the starting address
    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.addr_raw())
    }

    /// Returns the address as is
    pub const fn addr_raw(&self) -> u64 {
        (self.0 & Self::MASK_RETRIEVE_ADDR) >> 16
    }

    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn as_array(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }

    #[inline]
    pub const fn is_valid_size(size: u32) -> bool {
        size > 0 && size == size & (Self::MASK_RETRIEVE_SIZE >> 8) as u32
    }
}

impl From<u64> for LayoutTableEntry {
    fn from(value: u64) -> Self {
        LayoutTableEntry(value)
    }
}

impl From<LayoutTableEntry> for Arena {
    fn from(entry: LayoutTableEntry) -> Self {
        unsafe {
            let ptr = NonNull::new_unchecked(entry.addr().as_mut_ptr::<u8>());
            let size = NonZeroUsize::new(entry.size() as usize).unwrap();
            let capacity = AlignedNonZeroUsize::new_aligned_unchecked(size);

            Arena { ptr, capacity }
        }
    }
}

#[cfg(feature = "vmi-consume")]
impl Display for LayoutTableEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let addr = self.addr_raw() as usize;
        let size = self.len() as usize;
        let system = self.flags().contains(Flags::SYSTEM);
        let present = self.flags().contains(Flags::PRESENT);

        let usage = match () {
            _ if self.flags().contains(Flags::STACK) => "STACK",
            _ if self.flags().contains(Flags::CODE) => "CODE",
            _ if self.flags().contains(Flags::DATA) => match () {
                _ if self.flags().contains(Flags::READ) => "DATA - READ",
                _ if self.flags().contains(Flags::WRITE) => "DATA - WRITE",
                _ if self.flags().contains(Flags::SHARED_OWNED) => "DATA - SHARED OWNED",
                _ if self.flags().contains(Flags::SHARED_FOREIGN) => "DATA - SHARED FOREIGN",
                _ => "DATA - UNKNOWN",
            },
            _ => "",
        };

        write!(
            f,
            "Present: {} Addr: {:#x} Size: {} System: {} USAGE: {} ",
            present, addr, size, system, usage,
        )
    }
}

impl Zero for LayoutTableEntry {
    fn zero(&mut self) {
        self.0 = 0;
    }
}

mod test {
    #![allow(unused)]
    use super::*;

    #[test]
    fn layout_table_entry_new() {
        let addr = PhysAddr::new_unchecked(0x0000_1234_5678_9000);
        let entry = LayoutTableEntry::new(addr, 0x1234, Flags::empty());
        assert_eq!(0x1234567890123400, entry.0, "got {:x}", entry.0);
        assert_eq!(
            addr.as_u64(),
            entry.addr_raw(),
            "got {:x}",
            entry.addr().as_u64()
        );
        assert_eq!(Flags::empty().bits(), entry.flags().bits());
        assert_eq!(0x1234, entry.len());
        assert!(!entry.is_present());
    }

    #[test]
    fn layout_table_entry_build() {
        let mut entry = LayoutTableEntry::empty();
        entry
            .set_len(0xabcde)
            .set_flags(Flags::CODE | Flags::PRESENT)
            .set_addr(PhysAddr::new_unchecked(0x0000123456789000)); // check for correct truncation
        assert_eq!(0x123456789abcde09, entry.0, "got {:x}", entry.0);
    }

    #[test]
    fn flag_build() {
        assert_eq!(Flags::empty().bits(), 0);
        assert_eq!(
            (Flags::PRESENT | Flags::DATA | Flags::SHARED_FOREIGN).bits(),
            0b0010_0001
        );
        assert!((Flags::PRESENT | Flags::DATA | Flags::SHARED_FOREIGN).contains(Flags::DATA));
        assert!(
            (Flags::PRESENT | Flags::DATA | Flags::SHARED_FOREIGN).contains(Flags::SHARED_FOREIGN)
        );
        assert!((Flags::PRESENT | Flags::DATA | Flags::READ).contains(Flags::READ));
    }

    #[test]
    #[cfg(feature = "vmi-consume")]
    fn layout_len() {
        let mut layout = LayoutTable::default();
        for i in 0..5 {
            layout.entries[i].set_present(true);
        }

        assert_eq!(5, layout.len_present())
    }
}
