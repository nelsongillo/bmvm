use crate::interprete::{Interpret, Zero};
use crate::mem::{Align, AlignedNonZeroUsize, Arena, DefaultAlign, PhysAddr};
use bitflags::bitflags;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
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
        const DATA_SHARED_FOREIGN = 0b10 << 4;
        const DATA_SHARED_OWNED = 0b11 << 4;

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
            0b10 => Some(DataAccessMode::SharedForeign),
            0b11 => Some(DataAccessMode::SharedOwned),
            _ => unreachable!(), // We've masked to 2 bits, so only 0-3 possible
        }
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
            DataAccessMode::SharedForeign => Flags::DATA_SHARED_FOREIGN,
            DataAccessMode::SharedOwned => Flags::DATA_SHARED_OWNED,
        };

        Ok(())
    }

    /// Converts the flags to their `PageTableFlags` representation.
    /// Note: The present flag will not be set by this method.
    pub fn to_page_table_flags(self) -> PageTableFlags {
        let mut pt_flags = PageTableFlags::PRESENT;

        // If the entry indicates that the page is a data page, check writeable
        if !self.contains(Flags::CODE) {
            pt_flags |= PageTableFlags::NO_EXECUTE;
            if self.contains(Flags::DATA_WRITE) {
                pt_flags |= PageTableFlags::WRITABLE;
            }
        }

        pt_flags
    }
}

/// Data access modes (only valid for data sections)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataAccessMode {
    Read,
    Write,
    SharedForeign,
    SharedOwned,
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
impl core::fmt::Display for LayoutTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let addr = self.addr_raw() as usize;
        let size = self.len() as usize;
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
            (Flags::PRESENT | Flags::DATA_SHARED_FOREIGN).bits(),
            0b0010_0001
        );
        assert!(!(Flags::PRESENT | Flags::DATA_SHARED_FOREIGN).is_code());
        assert_eq!(
            (Flags::PRESENT | Flags::DATA_SHARED_FOREIGN).data_access_mode(),
            Some(DataAccessMode::SharedForeign)
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

        flags
            .set_data_access_mode(DataAccessMode::SharedForeign)
            .unwrap();
        assert_eq!(
            flags.data_access_mode(),
            Some(DataAccessMode::SharedForeign)
        );

        flags
            .set_data_access_mode(DataAccessMode::SharedOwned)
            .unwrap();
        assert_eq!(flags.data_access_mode(), Some(DataAccessMode::SharedOwned));
    }

    #[test]
    fn test_data_access_on_code_section() {
        let mut flags = Flags::new();
        flags.set_code(true);

        assert_eq!(flags.data_access_mode(), None);
        assert!(flags.set_data_access_mode(DataAccessMode::Read).is_err());
    }
}
