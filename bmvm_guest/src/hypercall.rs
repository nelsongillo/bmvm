use bmvm_common::HYPERCALL_IO_PORT;
use bmvm_common::mem::Transport;
use bmvm_common::vmi::Signature;
use core::arch::asm;

pub unsafe fn execute(sig: Signature, transport: Transport) -> Transport {
    unsafe {
        let return_primary: u64;
        let return_secondary: u64;
        asm!(
        // prepare for hypercall execution
        "mov rbx, {func}", // Move function signature to EBX
        "mov r8, {ptr}", // Move primary to RCX
        "mov r9, {cap}", // Move secondary ro RDX
        // Trigger VM Exit -> Hypercall Execution
        "out dx, al",
        // Register Setup
        in("dx") HYPERCALL_IO_PORT,
        // data does not matter, as all we care about is the function signature and ptr offset
        in("al") 0x00u8,
        func = in(reg) sig,
        ptr = in(reg) transport.primary,
        cap = in(reg) transport.secondary,
        // Post VM Exit
        // Read return value offset ptr from EAX and construct OffsetPtr
        lateout("r8") return_secondary,
        lateout("r9") return_primary,
        );

        Transport {
            primary: return_primary,
            secondary: return_secondary,
        }
    }
}
