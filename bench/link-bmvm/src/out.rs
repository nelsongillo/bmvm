#![feature(prelude_import)]
#![feature(macro_metavar_expr_concat)]
#![no_std]
#![no_main]
#[macro_use]
extern crate core;
#[prelude_import]
use core::prelude::rust_2024::*;
extern crate core;
use bmvm_guest::{expose, host};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up1_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP1: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up1".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 49u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP1: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP1.0;
#[used]
static BMVM_CALL_META_SIG_UP1: u64 = BMVM_CALL_META_TUPLE_UP1.1;
#[unsafe(no_mangle)]
pub extern "C" fn up1_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up1();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up1() -> i32 {
    1
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up1_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP1,
    func: up1_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up2_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP2: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up2".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 50u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP2: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP2.0;
#[used]
static BMVM_CALL_META_SIG_UP2: u64 = BMVM_CALL_META_TUPLE_UP2.1;
#[unsafe(no_mangle)]
pub extern "C" fn up2_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up2();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up2() -> i32 {
    2
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up2_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP2,
    func: up2_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up3_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP3: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up3".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 51u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP3: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP3.0;
#[used]
static BMVM_CALL_META_SIG_UP3: u64 = BMVM_CALL_META_TUPLE_UP3.1;
#[unsafe(no_mangle)]
pub extern "C" fn up3_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up3();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up3() -> i32 {
    3
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up3_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP3,
    func: up3_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up4_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP4: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up4".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 52u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP4: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP4.0;
#[used]
static BMVM_CALL_META_SIG_UP4: u64 = BMVM_CALL_META_TUPLE_UP4.1;
#[unsafe(no_mangle)]
pub extern "C" fn up4_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up4();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up4() -> i32 {
    4
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up4_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP4,
    func: up4_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up5_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP5: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up5".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 53u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP5: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP5.0;
#[used]
static BMVM_CALL_META_SIG_UP5: u64 = BMVM_CALL_META_TUPLE_UP5.1;
#[unsafe(no_mangle)]
pub extern "C" fn up5_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up5();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up5() -> i32 {
    5
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up5_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP5,
    func: up5_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up6_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP6: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up6".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 54u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP6: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP6.0;
#[used]
static BMVM_CALL_META_SIG_UP6: u64 = BMVM_CALL_META_TUPLE_UP6.1;
#[unsafe(no_mangle)]
pub extern "C" fn up6_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up6();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up6() -> i32 {
    6
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up6_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP6,
    func: up6_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up7_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP7: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up7".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 55u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP7: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP7.0;
#[used]
static BMVM_CALL_META_SIG_UP7: u64 = BMVM_CALL_META_TUPLE_UP7.1;
#[unsafe(no_mangle)]
pub extern "C" fn up7_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up7();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up7() -> i32 {
    7
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up7_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP7,
    func: up7_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_up8_a2d133c8dc27963d: [u8; 0] = [];
#[used]
static BMVM_CALL_META_TUPLE_UP8: ([u8; 17usize], u64) = {
    let param_hash = <() as bmvm_guest::TypeSignature>::SIGNATURE;
    let mut sig_hasher = bmvm_guest::SignatureHasher::new();
    sig_hasher.write("up8".as_bytes());
    sig_hasher.write(param_hash.to_le_bytes().as_slice());
    sig_hasher.write(
        <i32 as bmvm_guest::TypeSignature>::SIGNATURE
            .to_le_bytes()
            .as_slice(),
    );
    let sig = sig_hasher.finish();
    let sig_bytes = sig.to_ne_bytes();
    let meta_suffix = [117u8, 112u8, 56u8, 0u8, 0u8, 105u8, 51u8, 50u8, 0u8];
    let mut out = [0u8; 17usize];
    let mut i = 0;
    while i < 8 {
        out[i] = sig_bytes[i];
        i += 1;
    }
    let mut j = 0;
    while j < 9usize {
        out[i + j] = meta_suffix[j];
        j += 1;
    }
    (out, sig)
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.expose")]
static BMVM_CALL_META_UP8: [u8; 17usize] = BMVM_CALL_META_TUPLE_UP8.0;
#[used]
static BMVM_CALL_META_SIG_UP8: u64 = BMVM_CALL_META_TUPLE_UP8.1;
#[unsafe(no_mangle)]
pub extern "C" fn up8_bmvm_wrapper_a2d133c8dc27963d() {
    let __ret = up8();
    use bmvm_guest::OwnedShareable;
    let __output = __ret.into_transport();
    let __exit_code: u8 = bmvm_guest::ExitCode::Return.into();
    unsafe {
        asm!(
            "out dx, al", in ("dx") bmvm_guest::EXIT_IO_PORT, in ("al") __exit_code, in
            ("r8") __output.primary(), in ("r9") __output.secondary(), options(nomem,
            preserves_flags, noreturn, nostack)
        );
    }
}
#[inline]
pub fn up8() -> i32 {
    8
}
#[used]
#[allow(non_upper_case_globals)]
#[unsafe(link_section = ".bmvm.vmi.expose.calls")]
static UPCALL_FN_WRAPPER_up8_a2d133c8dc27963d: bmvm_guest::UpcallFn = bmvm_guest::UpcallFn {
    sig: BMVM_CALL_META_SIG_UP8,
    func: up8_bmvm_wrapper_a2d133c8dc27963d,
};
#[used]
#[unsafe(link_section = ".bmvm.vmi.debug")]
static BMVM_CALL_META_DEBUG_INDICATOR_a2d133c8dc27aa97_a2d133c8dc27aa97: [u8; 0] = [];
