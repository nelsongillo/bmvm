#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use core::slice;

use talc::*;

const SIZE: usize = 2 * 1024 * 1024;
static mut ARENA: [u8; SIZE] = [0; SIZE];

#[global_allocator]
static ALLOCATOR: Talck<spin::Mutex<()>, ClaimOnOom> =
    Talc::new(unsafe { ClaimOnOom::new(Span::from_array(core::ptr::addr_of!(ARENA).cast_mut())) })
        .lock();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn noop() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn reverse(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unsafe { slice::from_raw_parts(ptr, len) };

    let mut buf = Vec::with_capacity(len);
    buf.extend_from_slice(input);
    buf.reverse();

    let mut boxed = buf.into_boxed_slice();
    let out = boxed.as_mut_ptr();
    core::mem::forget(boxed);
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    core::mem::forget(buf); // don't free it, WASM owns it now
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(ptr: *mut u8, len: usize) {
    let _ = unsafe { Vec::from_raw_parts(ptr, len, len) };
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unreachable!();
}
