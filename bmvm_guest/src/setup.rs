use bmvm_common::error::ExitCode;
use bmvm_common::interprete::{Interpret, InterpretError};
use bmvm_common::mem::{Align, Arena, DataAccessMode, LayoutTable, Page4KiB};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, mem};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Parse the memory info structure and initialize the paging system etc.
#[inline(never)]
pub(super) fn setup() -> Result<(), ExitCode> {
    use raw_cpuid::CpuId;
    let cpuid = CpuId::new();
    if let Some(finfo) = cpuid.get_feature_info() {
        if !finfo.has_sse() {
            panic!("SSE!");
        }
        if !finfo.has_sse2() {
            panic!("SSE2!");
        }
        if !finfo.has_sse3() {
            panic!("SSE3!");
        }
        if !finfo.has_sse41() {
            panic!("SSE41!");
        }
        if !finfo.has_sse42() {
            panic!("SSE42!");
        }
    } else {
        panic!("Could not fetch CPU Features");
    }

    // https://github.com/rust-osdev/x86_64/blob/master/testing/tests/double_fault_stack_overflow.rs
    lazy_static::lazy_static! {
        static ref TEST_IDT: InterruptDescriptorTable = {
            let mut idt = InterruptDescriptorTable::new();

            x86_64::set_general_handler!(&mut idt, general_fault_handler, 0..32);
            idt
        };
    }
    TEST_IDT.load();

    let raw_ptr = BMVM_MEM_LAYOUT_TABLE.as_u64() as *const u8;
    let raw = unsafe { core::slice::from_raw_parts(raw_ptr, Page4KiB::ALIGNMENT as usize) };
    let table = LayoutTable::from_bytes(raw).map_err(|interpret_err| match interpret_err {
        InterpretError::TooSmall(_, _) => ExitCode::InvalidMemoryLayoutTableTooSmall,
        InterpretError::Misaligned(_, _) => ExitCode::InvalidMemoryLayoutTableMisaligned,
    })?;

    let shared = table
        .into_iter()
        .find(|entry| {
            entry
                .flags()
                .data_access_mode()
                .is_some_and(|m| m == DataAccessMode::Shared)
        })
        .map(Arena::from);

    // set up the allocator for the VMI
    mem::init(shared);

    Ok(())
}

#[inline(never)]
fn general_fault_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    super::panic::exit_with_code(ExitCode::Interrupt(index));
    loop {}
}
