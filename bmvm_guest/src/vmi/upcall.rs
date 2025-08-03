/*
pub fn execute() {
    let signature: u64;
    let primary: u64;
    let secondary: u64;

    unsafe {
        // Populate registers with the function signature and data offset ptr
        asm!(
        "mov rbx, {func}",
        "mov r8, {ptr}",
        "mov r9, {cap}",
        func = out(reg) signature,
        ptr = out(reg) primary,
        cap = out(reg) secondary,
        );
    }

    // try finding a related function by signature
    let params = Transport { primary, secondary };
    let calls = upcalls();
    let found = calls.binary_search_by_key(&signature, |upcall| upcall.sig);
    if found.is_err() {
        // No upcall found with provided signature
        exit_with_code(ExitCode::UnknownUpcall(signature));
    }

    // get Upcall via Index
    let upcall = &calls[found.unwrap()];
    // upcall execution
    let ret = (upcall.func)(params);

    // halt to indicate Host end of execution and propagate upcall return
    unsafe {
        asm!(
        "hlt",
        in("r8") ret.primary,
        in("r9") ret.secondary,
        );
    }
}
*/
