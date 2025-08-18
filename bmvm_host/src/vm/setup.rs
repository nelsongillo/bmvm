use bmvm_common::mem::{AddrSpace, Align, DefaultAddrSpace, DefaultAlign, align_ceil};
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

const EXT_PROCESSOR_INFO_INDEX: u32 = 0x80000008;
const EXT_PROCESSOR_INFO_EAX: u32 = 0x80000001;

pub(super) const GDT_LIMIT: u64 = 0xFF_FFFF;
pub(super) const GDT_BASE: u64 = 0;
pub(super) const GDT_ACCESS_CODE: u8 = 0x9B;
pub(super) const GDT_FLAGS_CODE: u8 = 0b1010;
pub(super) const GDT_ACCESS_DATA: u8 = 0x93;
pub(super) const GDT_FLAGS_DATA: u8 = 0b1100;

pub(crate) fn cpuid(kvm: &Kvm) -> Result<CpuId> {
    // setup vcpu cpuid
    let mut cpuid = kvm
        .get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
        .map_err(|_| Error::CpuID)?;

    // modify extended processor info (0x80000008)
    for entry in cpuid.as_mut_slice().iter_mut() {
        match entry.function {
            // Basic CPUID information
            0x00000001 => {
                // EDX bits:
                // Bit 3 = PSE (Page Size Extension)
                // Bit 6 = PAE (Physical Address Extension)
                entry.edx |= (1 << 3) | (1 << 6);

                // ECX bits:
                // Bit 20 = NX (No-Execute bit support)
                entry.ecx |= 1 << 20;
            }

            // Extended CPUID information
            EXT_PROCESSOR_INFO_EAX => {
                // EDX bits:
                // Bit 29 = LM (Long Mode, 64-bit support)
                entry.edx |= 1 << 29;
            }

            // Address size information
            EXT_PROCESSOR_INFO_INDEX => {
                // EBX bits:
                // Bits 15:8 = physical address bits (set to 39)
                // Bits 7:0 = virtual address bits (keep at 48)
                entry.ebx = (entry.ebx & !(0xFF00)) | ((DefaultAddrSpace::bits() as u32) << 8); // Set physical to supported host address size
                entry.ebx = (entry.ebx & !(0x00FF)) | 0x30; // Keep virtual at 48 bits

                // Indicate 1GB page support
                // ECX bits:
                // Bit 26 = 1GB page support
                entry.ecx |= 1 << 26;
            }

            _ => continue,
        }
    }

    Ok(cpuid)
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
    gdt.extend_from_slice(&gdt_entry(
        GDT_BASE,
        GDT_LIMIT,
        GDT_ACCESS_CODE,
        GDT_FLAGS_CODE,
    ));
    gdt.extend_from_slice(&gdt_entry(
        GDT_BASE,
        GDT_LIMIT,
        GDT_ACCESS_DATA,
        GDT_FLAGS_DATA,
    ));
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
