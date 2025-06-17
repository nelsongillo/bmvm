use bmvm_common::error::ExitCode;
use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Trigger VM exit with the provided exit code
pub(crate) fn exit_with_code(code: ExitCode) -> ! {
    unsafe {
        asm!(
            "hlt",
            in("al") code as u8,
            options(nomem, nostack, preserves_flags),
        )
    }
    loop {}
}

/// Trigger VM exit with the provided exit code.
pub fn panic_with_code(code: ExitCode) -> ! {
    exit_with_code(code)
}

/// Stop the execution
pub fn halt() -> ! {
    exit_with_code(ExitCode::Normal)
}
