use bmvm_common::mem::{AlignedNonZeroUsize, ForeignBuf, SharedBuf, alloc_buf};
use bmvm_host::{ConfigBuilder, ModuleBuilder, linker};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;
use wasmtime::component::__internal::wasmtime_environ::object::ReadRef;
use wasmtime::{Engine, Instance, Module as WasmModule, Store};

const BMVM: &str = "../bench/binaries/bmvm-echo";
const WASM: &str = "../bench/binaries/wasm-echo.wasm";
const BMVM_STACK: usize = 32 * 1024 * 1024; // 32MiB

pub fn bmvm_echo_noop(c: &mut Criterion) {
    let path = PathBuf::from(BMVM);
    let mut group = c.benchmark_group("bmvm-echo");
    group.measurement_time(Duration::from_secs(10));

    let linker = linker::ConfigBuilder::new()
        .register_guest_function::<(), ()>("noop")
        .register_guest_function::<(SharedBuf,), ForeignBuf>("reverse")
        .build();

    let vm = ConfigBuilder::new().stack_size(AlignedNonZeroUsize::new_ceil(BMVM_STACK).unwrap());

    let mut module = ModuleBuilder::new()
        .with_path(&path)
        .configure_linker(linker)
        .configure_vm(vm)
        .build()
        .unwrap();

    let noop = module.get_upcall::<(), ()>("noop").unwrap();

    let reverse = module
        .get_upcall::<(SharedBuf,), ForeignBuf>("reverse")
        .unwrap();

    group.bench_function("noop", |b| {
        b.iter(|| {
            black_box({
                let _ = noop.call(&mut module, ());
            })
        })
    });

    group.bench_function("reverse-64", |b| {
        b.iter(|| {
            black_box({
                let owned = unsafe { alloc_buf(64).unwrap() };
                let _ = reverse.call(&mut module, (owned.into_shared(),)).unwrap();
            })
        })
    });

    group.bench_function("reverse-256", |b| {
        b.iter(|| {
            black_box({
                let owned = unsafe { alloc_buf(256).unwrap() };
                let _ = reverse.call(&mut module, (owned.into_shared(),)).unwrap();
            })
        })
    });

    group.bench_function("reverse-1024", |b| {
        b.iter(|| {
            black_box({
                let owned = unsafe { alloc_buf(1024).unwrap() };
                let _ = reverse.call(&mut module, (owned.into_shared(),)).unwrap();
            })
        })
    });
}

pub fn wasm_echo_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm-echo");
    group.measurement_time(Duration::from_secs(5));

    let buf = std::fs::read(WASM).unwrap();

    let engine = Engine::default();
    let module = WasmModule::from_binary(&engine, buf.as_slice()).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let noop = instance
        .get_typed_func::<(), ()>(&mut store, "noop")
        .unwrap();

    let reverse = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "reverse")
        .unwrap();

    let alloc = instance
        .get_typed_func::<(i32,), i32>(&mut store, "alloc")
        .unwrap();

    let free = instance
        .get_typed_func::<(i32, i32), ()>(&mut store, "free")
        .unwrap();

    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("no memory export");

    group.bench_function("noop", |b| {
        b.iter(|| {
            black_box({
                let _ = noop.call(&mut store, ());
            })
        })
    });

    group.bench_function("reverse-64", |b| {
        let mut buf = [0u8; 64];
        b.iter(|| {
            black_box({
                // buffer prep + copy to wasm memory
                let ptr = alloc.call(&mut store, (64,)).unwrap();
                memory
                    .write(&mut store, ptr as usize, buf.as_slice())
                    .unwrap();
                // call wasm function and get result
                let rev = reverse.call(&mut store, (ptr as i32, 64)).unwrap();
                memory
                    .read(&mut store, rev as usize, buf.as_mut_slice())
                    .unwrap();
                // free wasm memory to restore original state
                free.call(&mut store, (ptr as i32, 64)).unwrap();
                free.call(&mut store, (rev as i32, 64)).unwrap();
            })
        })
    });

    group.bench_function("reverse-512", |b| {
        let mut buf = [0u8; 512];
        b.iter(|| {
            black_box({
                // buffer prep + copy to wasm memory
                let ptr = alloc.call(&mut store, (512,)).unwrap();
                memory
                    .write(&mut store, ptr as usize, buf.as_slice())
                    .unwrap();
                // call wasm function and get result
                let rev = reverse.call(&mut store, (ptr as i32, 512)).unwrap();
                memory
                    .read(&mut store, rev as usize, buf.as_mut_slice())
                    .unwrap();
                // free wasm memory to restore original state
                free.call(&mut store, (ptr as i32, 512)).unwrap();
                free.call(&mut store, (rev as i32, 512)).unwrap();
            })
        })
    });

    group.bench_function("reverse-1024", |b| {
        let mut buf = [0u8; 1024];
        b.iter(|| {
            black_box({
                // buffer prep + copy to wasm memory
                let ptr = alloc.call(&mut store, (1024,)).unwrap();
                memory
                    .write(&mut store, ptr as usize, buf.as_slice())
                    .unwrap();
                // call wasm function and get result
                let rev = reverse.call(&mut store, (ptr, 1024)).unwrap();
                memory
                    .read(&mut store, rev as usize, buf.as_mut_slice())
                    .unwrap();
                // free wasm memory to restore original state
                free.call(&mut store, (ptr as i32, 1024)).unwrap();
                free.call(&mut store, (rev as i32, 1024)).unwrap();
            })
        })
    });
}

pub fn native_echo_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("native-echo");
    group.measurement_time(Duration::from_secs(10));

    fn noop() {}

    fn reverse(buf: &[u8]) -> Vec<u8> {
        let mut rev = Vec::with_capacity(buf.len());
        rev.extend_from_slice(buf);
        rev.reverse();
        rev
    }

    group.bench_function("noop", |b| b.iter(|| black_box(noop())));

    group.bench_function("reverse-64", |b| {
        let buf = [0u8; 64];
        b.iter(|| black_box(reverse(&buf)))
    });

    group.bench_function("reverse-512", |b| {
        let buf = [0u8; 512];
        b.iter(|| black_box(reverse(&buf)))
    });

    group.bench_function("reverse-1024", |b| {
        let buf = [0u8; 1024];
        b.iter(|| black_box(reverse(&buf)))
    });
}

criterion_group!(benches, wasm_echo_noop, native_echo_noop, bmvm_echo_noop);
criterion_main!(benches);
