unsafe extern "Rust" {
    fn __process_entry();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        __process_entry();
    }
    loop {}
}
