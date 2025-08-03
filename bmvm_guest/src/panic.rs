use bmvm_common::error::ExitCode;
use bmvm_common::mem::Transport;
use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    panic_with_code(ExitCode::Unmapped(u8::MAX));
    loop {}
}

/// Trigger VM exit with the provided exit code
pub fn exit_with_code(code: ExitCode) -> ! {
    write_additional_values(code);
    unsafe {
        asm!(
            "hlt",
            in("al") code.as_u8(),
            options(nomem, nostack, preserves_flags),
        );
        loop {}
    }
}

pub fn exit_with_transport(transport: Transport) -> ! {
    unsafe {
        asm!(
        "hlt",
        in("al") ExitCode::Normal.as_u8(),
        in("r8") transport.primary,
        in("r9") transport.secondary,
        options(nomem, nostack, preserves_flags),
        );
        loop {}
    }
}

/// Trigger VM exit with the provided exit code.
pub fn panic_with_code(code: ExitCode) {
    exit_with_code(code)
}

/// Stop the execution
pub fn halt() -> ! {
    exit_with_code(ExitCode::Normal);
}

/// Write additional values to registers before VM exit.
fn write_additional_values(code: ExitCode) {
    unsafe {
        match code {
            ExitCode::UnknownUpcall(sig) => asm!("mov rbx, {}", in(reg) sig),
            ExitCode::Unmapped(code) => asm!("mov bl, {}", in(reg_byte) code),
            _ => {}
        }
    }
}
