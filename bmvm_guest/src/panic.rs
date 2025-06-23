use bmvm_common::error::ExitCode;
use bmvm_common::error::ExitCode::Unmapped;
use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    panic_with_code(Unmapped(u8::MAX));
    loop {}
}

/// Trigger VM exit with the provided exit code
pub fn exit_with_code(code: ExitCode) {
    unsafe {
        asm!(
            "hlt",
            in("al") code.as_u8(),
            options(nomem, nostack, preserves_flags),
        )
    }
}

/// Trigger VM exit with the provided exit code.
pub fn panic_with_code(code: ExitCode) {
    exit_with_code(code)
}

/// Stop the execution
pub fn halt() -> ! {
    exit_with_code(ExitCode::Normal);
    loop {}
}
