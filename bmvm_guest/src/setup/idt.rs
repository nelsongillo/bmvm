use crate::exit_with_code;
use bmvm_common::error::ExitCode;
use bmvm_common::mem::LayoutTableEntry;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub(crate) fn setup(sys: &LayoutTableEntry, offset: u64) -> Result<(), ExitCode> {
    // let idt_ptr = (sys.addr_raw() + offset) as *mut InterruptDescriptorTable;
    // let idt = unsafe { &mut *idt_ptr };
    // set_general_handler!(idt, handler_upcall_irq);
    Ok(())
}

fn handler_upcall_irq(_stack_frame: InterruptStackFrame, _index: u8, _error_code: Option<u64>) {
    // todo!("handle irq {}", index)

    // let upcall_sig: u64;
    // let ptr: u32;
    // unsafe {
    //     asm!(
    //         "mov rbx, {0}",
    //         "mov ecx, {1:e}",
    //         out(reg) upcall_sig,
    //         out(reg) ptr,
    //     )
    // }

    // let ret = vmi::upcall(upcall_sig, RawOffsetPtr::from(ptr));
    exit_with_code(ExitCode::Normal)
}
