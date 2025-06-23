use std::arch::x86_64::__cpuid;

fn main() {
    let result = unsafe { __cpuid(0x80000008) };
    println!("RAX: {}, RBX: {}", result.eax, result.ebx);
}
