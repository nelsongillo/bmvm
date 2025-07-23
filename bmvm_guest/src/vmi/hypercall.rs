use crate::vmi::HYPERCALL_IO_PORT;
use bmvm_common::TypeHash;
use bmvm_common::mem::OffsetPtr;
use bmvm_common::vmi::Signature;
use core::arch::asm;

pub unsafe fn execute<P, R>(func: Signature, ptr: OffsetPtr<P>) -> OffsetPtr<R>
where
    P: TypeHash,
    R: TypeHash,
{
    unsafe {
        // Populate registers with the function signature and data offset ptr
        asm!(
        "mov {func}, ebx", // Move function signature to EBX
        "mov  {ptr}, ecx", // Move ptr offset value to ECX
        ptr = in(reg) ptr.offset,
        func = in(reg) func,
        );

        let return_ptr: u32;
        asm!(
            // Trigger VM Exit
            "outb al, dx",
            in("dx") HYPERCALL_IO_PORT,
            // data does not matter, as all we care about is the function signature and ptr offset
            in("al") 0x00u8,
            // Post VM Exit
            // Read return value offset ptr from EAX and construct OffsetPtr
            lateout("eax") return_ptr,
        );

        OffsetPtr::from(return_ptr)
    }
}
