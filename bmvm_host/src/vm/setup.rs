use bmvm_common::mem::{Align, DefaultAlign, align_ceil};
use kvm_bindings::{CpuId, KVM_MAX_CPUID_ENTRIES};
use kvm_ioctls::Kvm;

// Values used for system region requirement estimation
// ------------------------------------------------------------------------------------------------
pub(super) const IDT_SIZE: u64 = 0x1000;
pub(super) const GDT_SIZE: u64 = 0x1000;
pub(super) const GDT_ENTRY_SIZE: usize = 8;
pub(super) const IDT_ENTRY_SIZE: usize = 8;
pub const IDT_PAGE_REQUIRED: usize = (align_ceil(IDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;
pub const GDT_PAGE_REQUIRED: usize = (align_ceil(GDT_SIZE) / DefaultAlign::ALIGNMENT) as usize;

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Empty Module")]
    EmptyModule,
    #[error("Invalid argument")]
    CpuID,
}

pub(crate) fn cpuid(kvm: &Kvm) -> Result<CpuId> {
    // setup vcpu cpuid
    kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
        .map_err(|_| Error::CpuID)
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
    gdt.extend_from_slice(&gdt_entry(0, 0xF_FFFF, 0x9A, 0b1010));
    gdt.extend_from_slice(&gdt_entry(0, 0xF_FFFF, 0x92, 0b1010));
    gdt
}

/// Constructs a new GDT entry
#[inline]
const fn gdt_entry(base: u64, limit: u64, access_byte: u8, flags: u8) -> [u8; GDT_ENTRY_SIZE] {
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
