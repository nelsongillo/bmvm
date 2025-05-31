use anyhow::anyhow;
use crate::interprete::{Interpret, Zero};
use crate::mem::{Align, DefaultAlign, PhysAddr};
use bitflags::bitflags;
use x86_64::structures::paging::PageTableFlags;

#[repr(C)]
pub struct LayoutTable {
    pub entries: [LayoutTableEntry; 512],
}

impl LayoutTable {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "std")]
    pub fn from_vec(vec: &Vec<LayoutTableEntry>) -> anyhow::Result<LayoutTable> {
        if vec.len() > 512 {
            return Err(anyhow!("laoyut table cannot contain more than 512 entries: got {}", vec.len()))
        }
        let mut l = LayoutTable::new();
        let idx = 0;
        for e in vec.iter() {
            l.entries[idx] = *e;
        }
        Ok(l)
    }

    #[cfg(feature = "std")]
    pub fn as_vec(&self) -> Vec<LayoutTableEntry> {
        self.entries
            .iter()
            .filter(|e| e.is_present())
            .map(|e| e.clone())
            .collect::<Vec<_>>()
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
    pub struct Flags: u8 {
        ///  0 -> User; 1 -> System structures
        const SYSTEM = 0b0000_0001;
        /// 0 -> Data; 1 -> Code/Executable
        const CODE = 0b0000_0010;
        /// 0 -> Read; 1 -> Write
        const WRITE = 0b0000_0100;
        // const have no real purpose besides making it more explicit to create
        // eg: a user read only data section. Using USER | READ | DATA is the same as Flags::empty()
        // but more explicit.
        const USER = 0b0000_0000;
        const READ = 0b0000_0000;
        const DATA = 0b0000_0000;

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

impl TryFrom<&str> for Flags {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            ".text" => Ok(Self::CODE),                  // Executable code
            ".rodata" => Ok(Self::DATA | Self::READ),   // Read-only constants/data
            ".eh_frame" => Ok(Self::DATA | Self::READ), // Exception handling tables (read-only)
            ".data" => Ok(Self::WRITE),                 // Initialized writable data
            ".bss" => Ok(Self::WRITE),                  // Uninitialized data (zero-filled)
            _ => Err("unknown flag"),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub struct LayoutTableEntry(u64);

/// The layout table entry is 64 bits wide, so we can use the first 63 bits for the size
/// 0: Present bit - if set, the entry is valid
/// 1: 0 -> User; 1 -> System structures
/// 2: 0 -> Data; 1 -> Code/Executable
/// 3: If Section is Data:
///     1 -> Write
///     0 -> Read
///    Else:
///     Unused
/// 8-24: multiplicator of pages
/// 24-63: starting address
impl LayoutTableEntry {
    const MASK_PRESENT: u64 = 1;
    const MASK_FLAGS: u64 = 0xfe;
    const MASK_SIZE: u64 = 0xffff00;
    const MASK_ADDR: u64 = 0xffff_ffff_ff00_0000;

    /// Creates a new LayoutTableEntry with the given parameters.
    ///
    /// # Parameters
    pub fn new(addr: PhysAddr, size: u16, flags: Flags) -> Self {
        assert_ne!(0, size, "size must not be zero");
        // init with the present bit set
        let mut value = 0 | Self::MASK_PRESENT;

        // set the flags
        value |= (flags.bits() as u64) << 1;

        // set the size
        value |= (size as u64) << 8;
        // set the address
        value |= (addr.as_u64()) << 12;

        LayoutTableEntry(value)
    }

    /// Creates a new empty layoutTableEntry
    pub fn empty() -> Self {
        LayoutTableEntry(0)
    }

    pub fn set_present(mut self, present: bool) -> Self {
        if present {
            self.0 |= Self::MASK_PRESENT;
        } else {
            self.0 &= !Self::MASK_PRESENT;
        }
        self
    }

    pub fn set_flags(mut self, flags: Flags) -> Self {
        self.0 &= !Self::MASK_FLAGS;
        self.0 |= (flags.bits() as u64) << 1;
        self
    }

    pub fn set_len(mut self, size: u16) -> Self {
        self.0 &= !Self::MASK_SIZE;
        self.0 |= (size as u64) << 8;
        self
    }

    pub fn set_addr(mut self, addr: PhysAddr) -> Self {
        self.0 &= !Self::MASK_ADDR;
        self.0 |= (addr.as_u64()) << 12;
        self
    }

    /// Checks if the entry is present
    pub fn is_present(&self) -> bool {
        (self.0 & Self::MASK_PRESENT) > 0
    }

    pub fn flags(&self) -> Flags {
        let f = (self.0 & Self::MASK_FLAGS) > 1;
        Flags::from_bits_retain(f as u8)
    }

    /// Gets the amount of pages included in the entry
    pub fn len(&self) -> u16 {
        ((self.0 & Self::MASK_SIZE) >> 8) as u16
    }

    /// Returns the size of the entry in bytes
    pub fn size(&self) -> u64 {
        self.len() as u64 * DefaultAlign::ALIGNMENT
    }

    /// Returns the starting address
    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new((self.0 & Self::MASK_ADDR) >> 12)
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
        let addr = PhysAddr::new(0x000123456789a000);
        let entry = LayoutTableEntry::new(addr, 0x1234, Flags::empty());
        assert_eq!(0x123456789a123401, entry.0, "got {:x}", entry.0);
        assert_eq!(
            addr.as_u64(),
            entry.addr().as_u64(),
            "got {:x}",
            entry.addr().as_u64()
        );
        assert_eq!(Flags::empty().bits(), entry.flags().bits());
        assert_eq!(0x1234, entry.size());
        assert!(entry.is_present());
    }

    #[test]
    fn layout_table_entry_build() {
        let entry = LayoutTableEntry::empty()
            .set_present(true)
            .set_len(0x1234)
            .set_flags(Flags::CODE)
            .set_addr(PhysAddr::new(0x000123456789a123)); // check for correct truncation
        assert_eq!(0x123456789a123403, entry.0, "got {:x}", entry.0);
    }
}
