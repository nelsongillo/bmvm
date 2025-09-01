use crate::interprete::{Interpret, Zero};
use crate::mem::{Align, AlignedNonZeroUsize, Arena, DefaultAlign, PhysAddr, VirtAddr};
use bitflags::bitflags;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

pub const MAX_REGION_SIZE: u64 = u16::MAX as u64 * DefaultAlign::ALIGNMENT;

#[repr(C)]
pub struct LayoutTable {
    pub entries: [LayoutTableEntry; 256],
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
            entries: [LayoutTableEntry::empty(); 256],
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

impl<'a> IntoIterator for &'a LayoutTable {
    type Item = LayoutTableEntry;
    type IntoIter = LayoutTableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        LayoutTableIter { iter: self, idx: 0 }
    }
}

bitflags! {
    /// Memory entry flags with precise bit layout
    ///
    /// Bit layout:
    /// - 0: Present bit - if set, the entry is valid
    /// - 1: 0 -> User; 1 -> System structures
    /// - 2: Stack
    /// - 3: 0 -> Data; 1 -> Code/Executable
    /// - 4-5:
    ///     If Section is Data:
    ///         00 -> Read
    ///         01 -> Write
    ///         10 -> Shared Foreign
    ///         11 -> Shared Owned
    ///     Else: Unused
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u8 {
        /// Present bit - if set, the entry is valid
        const PRESENT = 1 << 0;
        /// System structures (when set), User (when not set)
        const SYSTEM = 1 << 1;
        /// Stack flag
        const STACK = 1 << 2;
        /// Code/Executable (when set), Data (when not set)
        const CODE = 1 << 3;

        // Data-specific access flags (bits 4-5)
        const DATA_READ = 0b00 << 4;
        const DATA_WRITE = 0b01 << 4;
        const DATA_SHARED = 0b11 << 4;

        // Mask for data access bits
        const DATA_ACCESS_MASK = 0b11 << 4;
    }
}

impl Flags {
    /// Create new flags with all bits cleared
    pub fn new() -> Self {
        Flags::empty()
    }

    /// Check if this is a valid (present) entry
    pub fn is_present(&self) -> bool {
        self.contains(Flags::PRESENT)
    }

    /// Set the present bit
    pub fn set_present(&mut self, present: bool) {
        self.set(Flags::PRESENT, present);
    }

    /// Check if this is a system entry
    pub fn is_system(&self) -> bool {
        self.contains(Flags::SYSTEM)
    }

    /// Set system/user flag
    pub fn set_system(&mut self, system: bool) {
        self.set(Flags::SYSTEM, system);
    }

    /// Check if this is a stack entry
    pub fn is_stack(&self) -> bool {
        self.contains(Flags::STACK)
    }

    /// Set stack flag
    pub fn set_stack(&mut self, stack: bool) {
        self.set(Flags::STACK, stack);
    }

    /// Check if this is code (executable)
    pub fn is_code(&self) -> bool {
        self.contains(Flags::CODE)
    }

    /// Set code/data flag
    pub fn set_code(&mut self, code: bool) {
        self.set(Flags::CODE, code);
    }

    /// Get the data access mode (only valid when !is_code())
    pub fn data_access_mode(&self) -> Option<DataAccessMode> {
        if self.is_code() {
            return None;
        }

        match self.bits() >> 4 & 0b11 {
            0b00 => Some(DataAccessMode::Read),
            0b01 => Some(DataAccessMode::Write),
            0b11 => Some(DataAccessMode::Shared),
            _ => panic!("Invalid data access mode"),
        }
    }

    pub fn is_write(&self) -> bool {
        if self.is_code() {
            return false;
        }
        if self.is_stack() {
            return true;
        }

        self.data_access_mode().is_some_and(|m| match m {
            DataAccessMode::Read => false,
            DataAccessMode::Write => true,
            DataAccessMode::Shared => true,
        })
    }

    /// Set the data access mode (only valid when !is_code())
    pub fn set_data_access_mode(&mut self, mode: DataAccessMode) -> Result<(), &'static str> {
        if self.is_code() {
            return Err("Cannot set data access mode on code section");
        }

        // Clear the existing bits
        self.remove(Flags::DATA_ACCESS_MASK);

        // Set the new bits
        *self |= match mode {
            DataAccessMode::Read => Flags::DATA_READ,
            DataAccessMode::Write => Flags::DATA_WRITE,
            DataAccessMode::Shared => Flags::DATA_SHARED,
        };

        Ok(())
    }
}

/// Data access modes (only valid for data sections)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataAccessMode {
    Read,
    Write,
    Shared,
}

#[cfg(feature = "vmi-consume")]
impl core::fmt::Display for DataAccessMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags::new()
    }
}
impl Flags {}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct LayoutTableEntry(u128);

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
/// 8-27: multiplicator of pages
/// 28-63: physical starting address
/// 64-99: virtual starting address
/// 100-127: padding
impl LayoutTableEntry {
    const MASK_RETRIEVE_FLAGS: u128 = 0xff;
    const MASK_RETRIEVE_SIZE: u128 = 0xf_ffff << 8;
    const MASK_RETRIEVE_PADDR: u128 = 0xf_ffff_ffff << 28;
    const MASK_RETRIEVE_VADDR: u128 = 0xf_ffff_ffff << 64;

    /// Creates a new LayoutTableEntry with the given parameters.
    ///
    /// # Parameters
    /// * `addr` - The physical address, where the region should start. Must be a valid paged aligned physical address.
    /// * `size` - Indicates the number of pages the region is spanning. Limited to 20bit, resulting in maximal 4GiB big regions.
    /// * `flags` - The flags mark the entry for a specific use-case, which will result in the equivalent paging flags.
    pub fn new(paddr: PhysAddr, vaddr: VirtAddr, size: u32, flags: Flags) -> Self {
        assert!(
            DefaultAlign::is_aligned(paddr.as_u64()),
            "physical addr must be page aligned"
        );
        assert!(
            DefaultAlign::is_aligned(vaddr.as_u64()),
            "virtual addr must be page aligned"
        );
        assert!(
            Self::is_valid_size(size),
            "size must not be zero and exceed 20 bits"
        );
        let mut value = flags.bits() as u128;

        // set the size
        value |= (size as u128) << 8;
        // set the physical address
        value |= (paddr.as_u64() as u128 & 0xFFFF_FFFF_F000) << 16;
        // set the virtual address
        value |= (vaddr.as_u64() as u128 & 0xFFFF_FFFF_F000) << 52;

        LayoutTableEntry(value)
    }

    /// Creates a new empty layoutTableEntry
    pub const fn empty() -> Self {
        LayoutTableEntry(0)
    }

    pub fn set_present(self, present: bool) -> Self {
        let mut flags = self.flags();
        flags.set(Flags::PRESENT, present);
        self.set_flags(flags);
        self
    }

    pub const fn set_flags(mut self, flags: Flags) -> Self {
        self.0 &= !Self::MASK_RETRIEVE_FLAGS;
        self.0 |= flags.bits() as u128;
        self
    }

    pub const fn set_len(mut self, size: u32) -> Self {
        self.0 &= !Self::MASK_RETRIEVE_SIZE;
        self.0 |= (size as u128) << 8;
        self
    }

    pub fn set_paddr(mut self, addr: PhysAddr) -> Self {
        assert!(
            DefaultAlign::is_aligned(addr.as_u64()),
            "physical addr must be page aligned"
        );
        self.0 &= !Self::MASK_RETRIEVE_PADDR;
        self.0 |= (addr.as_u64() as u128 & 0xFFFF_FFFF_F000) << 16;
        self
    }

    pub fn set_vaddr(mut self, addr: VirtAddr) -> Self {
        assert!(
            DefaultAlign::is_aligned(addr.as_u64()),
            "virtual addr must be page aligned"
        );
        self.0 &= !Self::MASK_RETRIEVE_VADDR;
        self.0 |= (addr.as_u64() as u128 & 0xFFFF_FFFF_F000) << 52;
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
    pub const fn pages(&self) -> u32 {
        ((self.0 & Self::MASK_RETRIEVE_SIZE) >> 8) as u32
    }

    /// Returns the size of the entry in bytes. The size is page-aligned.
    pub const fn size(&self) -> u64 {
        self.pages() as u64 * DefaultAlign::ALIGNMENT
    }

    /// Returns the physical starting address
    pub fn paddr(&self) -> PhysAddr {
        PhysAddr::new(self.paddr_raw())
    }

    /// Returns the virtual starting address
    pub fn vaddr(&self) -> VirtAddr {
        VirtAddr::new_truncate(self.vaddr_raw())
    }

    /// Returns the raw physical address as stored in the entry
    pub const fn paddr_raw(&self) -> u64 {
        ((self.0 & Self::MASK_RETRIEVE_PADDR) >> 16) as u64
    }

    /// Returns the raw virtual address as stored in the entry
    pub const fn vaddr_raw(&self) -> u64 {
        ((self.0 & Self::MASK_RETRIEVE_VADDR) >> 52) as u64
    }

    #[inline]
    pub const fn as_u128(&self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn as_array(&self) -> [u8; 16] {
        self.0.to_ne_bytes()
    }

    #[inline]
    pub const fn is_valid_size(size: u32) -> bool {
        size > 0 && size == size & (Self::MASK_RETRIEVE_SIZE >> 8) as u32
    }
}

impl From<u128> for LayoutTableEntry {
    fn from(value: u128) -> Self {
        LayoutTableEntry(value)
    }
}

impl From<LayoutTableEntry> for Arena {
    fn from(entry: LayoutTableEntry) -> Self {
        unsafe {
            let ptr = NonNull::new_unchecked(entry.vaddr().as_mut_ptr::<u8>());
            let size = NonZeroUsize::new(entry.size() as usize).unwrap();
            let capacity = AlignedNonZeroUsize::new_unchecked(size);

            Arena { ptr, capacity }
        }
    }
}

#[cfg(feature = "vmi-consume")]
impl core::fmt::Display for LayoutTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let paddr = self.paddr_raw() as usize;
        let vaddr = self.vaddr_raw() as usize;
        let size = self.pages() as usize;
        let system = self.flags().contains(Flags::SYSTEM);
        let present = self.flags().contains(Flags::PRESENT);

        let usage = match () {
            _ if self.flags().is_stack() => String::from("STACK"),
            _ if self.flags().is_code() => String::from("CODE"),
            _ if !self.flags().is_code() => format!("{}", self.flags().data_access_mode().unwrap()),
            _ => String::new(),
        };

        write!(
            f,
            "Present: {}, PhysAddr: {:#x}, VirtAddr: {:#x}, Size: {}, System: {}, USAGE: {} ",
            present, paddr, vaddr, size, system, usage,
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
        let paddr = PhysAddr::new_unchecked(0x0000_1234_5678_9000);
        let vaddr = VirtAddr::new_truncate(0x0000_1234_5678_9000);
        let empty_flags = Flags::empty();
        let size = 0x1234;
        let constructor = LayoutTableEntry::new(paddr, vaddr, size, empty_flags);
        let builder = LayoutTableEntry::empty()
            .set_vaddr(vaddr)
            .set_paddr(paddr)
            .set_len(size)
            .set_flags(empty_flags);

        assert_eq!(
            0x1234567891234567890123400, constructor.0,
            "got {:x}",
            builder.0
        );
        assert_eq!(
            constructor.0, builder.0,
            "expected {:x}, got {:x}",
            constructor.0, builder.0
        );
        assert_eq!(
            paddr.as_u64(),
            constructor.paddr_raw(),
            "paddr got {:x}",
            constructor.paddr().as_u64()
        );
        assert_eq!(
            vaddr.as_u64(),
            constructor.vaddr_raw(),
            "vaddr got {:x}",
            constructor.vaddr().as_u64()
        );
        assert_eq!(Flags::empty().bits(), constructor.flags().bits());
        assert_eq!(0x1234, constructor.pages());
        assert!(!constructor.is_present());
    }

    #[test]
    fn layout_table_entry_build() {
        let mut entry = LayoutTableEntry::empty()
            .set_len(0xabcde)
            .set_flags(Flags::CODE | Flags::PRESENT)
            .set_paddr(PhysAddr::new_unchecked(0x0000123456789000))
            .set_vaddr(VirtAddr::new_unchecked(0x0000123456789000)); // check for correct truncation
        let want: u128 = 0x123456789123456789abcde09;
        assert_eq!(want, entry.0, "wnat {:x} but got {:x}", want, entry.0);
    }

    #[test]
    fn flag_build() {
        assert_eq!(Flags::empty().bits(), 0);
        assert_eq!((Flags::PRESENT | Flags::DATA_SHARED).bits(), 0b0011_0001);
        assert!(!(Flags::PRESENT | Flags::DATA_SHARED).is_code());
        assert_eq!(
            (Flags::PRESENT | Flags::DATA_SHARED).data_access_mode(),
            Some(DataAccessMode::Shared)
        );
        assert_eq!(
            (Flags::PRESENT | Flags::DATA_READ).data_access_mode(),
            Some(DataAccessMode::Read)
        );
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

    #[test]
    fn test_present_flag() {
        let mut flags = Flags::new();
        assert!(!flags.is_present());

        flags.set_present(true);
        assert!(flags.is_present());
        assert_eq!(flags.bits(), 0b00000001);

        flags.set_present(false);
        assert!(!flags.is_present());
    }

    #[test]
    fn test_system_flag() {
        let mut flags = Flags::new();
        assert!(!flags.is_system());

        flags.set_system(true);
        assert!(flags.is_system());
        assert_eq!(flags.bits(), 0b00000010);
    }

    #[test]
    fn test_code_data_flag() {
        let mut flags = Flags::new();
        assert!(!flags.is_code());

        flags.set_code(true);
        assert!(flags.is_code());
        assert_eq!(flags.bits(), 0b00001000);
    }

    #[test]
    fn test_data_access_modes() {
        let mut flags = Flags::new();
        flags.set_code(false); // Ensure it's data

        flags.set_data_access_mode(DataAccessMode::Read).unwrap();
        assert_eq!(flags.data_access_mode(), Some(DataAccessMode::Read));

        flags.set_data_access_mode(DataAccessMode::Write).unwrap();
        assert_eq!(flags.data_access_mode(), Some(DataAccessMode::Write));

        flags.set_data_access_mode(DataAccessMode::Shared).unwrap();
        assert_eq!(flags.data_access_mode(), Some(DataAccessMode::Shared));

        flags.set_data_access_mode(DataAccessMode::Shared).unwrap();
        assert_eq!(flags.data_access_mode(), Some(DataAccessMode::Shared));
    }

    #[test]
    fn test_data_access_on_code_section() {
        let mut flags = Flags::new();
        flags.set_code(true);

        assert_eq!(flags.data_access_mode(), None);
        assert!(flags.set_data_access_mode(DataAccessMode::Read).is_err());
    }
}
