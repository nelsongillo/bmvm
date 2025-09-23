use criterion::{Criterion, criterion_group, criterion_main};
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;
use std::ffi::c_void;
use std::hint::black_box;
use std::io::Write;
use std::ptr::null_mut;
use std::slice;
use std::time::Duration;

use kvm_bindings::{kvm_regs, kvm_userspace_memory_region};

fn ctx_switching_pio(c: &mut Criterion) {
    let mem_size = 0x1000;
    let guest_addr = 0x1000;
    let asm_code = &[0xee; 0x1000];
    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };

    let slot = 0;
    let mem_region = kvm_userspace_memory_region {
        slot,
        guest_phys_addr: guest_addr,
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64,
        flags: 0,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    // Write the code in the guest memory. This will generate a dirty page.
    unsafe {
        let mut slice = slice::from_raw_parts_mut(load_addr, mem_size);
        slice.write(asm_code).unwrap();
    }

    let mut vcpu_fd = vm.create_vcpu(0).unwrap();
    // x86_64 specific registry setup.
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
    vcpu_sregs.cs.base = 0;
    vcpu_sregs.cs.selector = 0;
    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = guest_addr;
    vcpu_regs.rax = 2;
    vcpu_regs.rdx = 0x3f8;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    let mut group = c.benchmark_group("ctx-pio");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("pio", |b| {
        b.iter(|| {
            black_box({
                match vcpu_fd.run().expect("run failed") {
                    VcpuExit::IoOut(_, _) => {
                        vcpu_fd.set_regs(&vcpu_regs).unwrap();
                    }
                    r => panic!("Unexpected exit reason: {:?}", r),
                }
            })
        })
    });

    unsafe { libc::munmap(load_addr as *mut c_void, mem_size) };
}

fn ctx_switching_mmio(c: &mut Criterion) {
    let mem_size = 0x4000;
    let guest_addr = 0x1000;
    let asm_code: &[u8] = &[
        0xc6, 0x06, 0x00, 0x80, 0x00, /* movl $0, (0x8000); This generates a MMIO Write. */
    ];
    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };

    let slot = 0;
    let mem_region = kvm_userspace_memory_region {
        slot,
        guest_phys_addr: guest_addr,
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64,
        flags: 0,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    // Write the code in the guest memory. This will generate a dirty page.
    unsafe {
        let mut slice = slice::from_raw_parts_mut(load_addr, mem_size);
        slice.write(&asm_code).unwrap();
    }

    let mut vcpu_fd = vm.create_vcpu(0).unwrap();
    // x86_64 specific registry setup.
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
    vcpu_sregs.cs.base = 0;
    vcpu_sregs.cs.selector = 0;
    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = guest_addr;
    vcpu_regs.rax = 2;
    vcpu_regs.rdx = 3;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    let mut group = c.benchmark_group("ctx-mmio");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("mmio", |b| {
        b.iter(|| {
            black_box({
                match vcpu_fd.run().expect("run failed") {
                    VcpuExit::MmioWrite(_, _) => {
                        vcpu_fd.set_regs(&vcpu_regs).unwrap();
                    }
                    r => panic!("Unexpected exit reason: {:?}", r),
                }
            })
        })
    });

    unsafe { libc::munmap(load_addr as *mut c_void, mem_size) };
}

fn ctx_switching_halt(c: &mut Criterion) {
    let mem_size = 0x1000;
    let guest_addr = 0x1000;
    let asm_code: &[u8] = &[0xf4 /* hlt */];
    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };

    let slot = 0;
    let mem_region = kvm_userspace_memory_region {
        slot,
        guest_phys_addr: guest_addr,
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64,
        flags: 0,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    // Write the code in the guest memory. This will generate a dirty page.
    unsafe {
        let mut slice = slice::from_raw_parts_mut(load_addr, mem_size);
        slice.write(&asm_code).unwrap();
    }

    let mut vcpu_fd = vm.create_vcpu(0).unwrap();
    // x86_64 specific registry setup.
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();
    vcpu_sregs.cs.base = 0;
    vcpu_sregs.cs.selector = 0;
    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = guest_addr;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    let mut group = c.benchmark_group("halt");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("halt", |b| {
        b.iter(|| {
            black_box({
                match vcpu_fd.run().expect("run failed") {
                    VcpuExit::Hlt => {
                        vcpu_fd.set_regs(&vcpu_regs).unwrap();
                    }
                    r => panic!("Unexpected exit reason: {:?}", r),
                };
            })
        })
    });

    unsafe { libc::munmap(load_addr as *mut c_void, mem_size) };
}

fn ctx_switching_rm_vcpu_setting(c: &mut Criterion) {
    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    let vcpu_fd = vm.create_vcpu(0).unwrap();
    let mut group = c.benchmark_group("ctx-register-reset");
    group.measurement_time(Duration::from_secs(10));

    let vcpu_regs = kvm_regs {
        rip: 0x1000,
        ..Default::default()
    };

    group.bench_function("set", |b| {
        b.iter(|| {
            black_box({
                vcpu_fd.set_regs(&vcpu_regs).unwrap();
            })
        })
    });
}

criterion_group!(
    benches,
    ctx_switching_pio,
    ctx_switching_mmio,
    ctx_switching_halt,
    ctx_switching_rm_vcpu_setting
);
criterion_main!(benches);
