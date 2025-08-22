use bmvm_common::HYPERCALL_IO_PORT;
use bmvm_common::vmi::{Signature, Transport};
use core::arch::asm;

pub unsafe fn execute(sig: Signature, transport: Transport) -> Transport {
    unsafe {
        let mut primary: u64 = transport.primary();
        let mut secondary: u64 = transport.secondary();
        asm!(
            // prepare for hypercall execution
            "mov rbx, {func}",          // Move function signature to EBX
            "out dx, al",               // Trigger VM Exit -> Hypercall Execution (we do not cate about the data in al)
            func = in(reg) sig,
            in("dx") HYPERCALL_IO_PORT,
            // Post VM Exit
            // Read return value offset ptr from EAX and construct OffsetPtr
            inlateout("r8") primary,
            inlateout("r9") secondary,
        );

        Transport::new(primary, secondary)
    }
}
