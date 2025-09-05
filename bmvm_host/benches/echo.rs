use bmvm_common::mem::AlignedNonZeroUsize;
use bmvm_host::{ConfigBuilder, ModuleBuilder, linker};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;
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
        .build();

    let vm = ConfigBuilder::new().stack_size(AlignedNonZeroUsize::new_ceil(BMVM_STACK).unwrap());

    let mut module = ModuleBuilder::new()
        .with_path(&path)
        .configure_linker(linker)
        .configure_vm(vm)
        .build()
        .unwrap();

    let noop = module.get_upcall::<(), ()>("noop").unwrap();

    group.bench_function("noop", |b| {
        b.iter(|| {
            black_box({
                let _ = noop.call(&mut module, ());
            })
        })
    });
}

pub fn wasm_echo_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm-echo");
    group.measurement_time(Duration::from_secs(10));

    let buf = std::fs::read(WASM).unwrap();

    let engine = Engine::default();
    let module = WasmModule::from_binary(&engine, buf.as_slice()).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let noop = instance
        .get_typed_func::<(), ()>(&mut store, "noop")
        .unwrap();

    group.bench_function("noop", |b| {
        b.iter(|| {
            black_box({
                let _ = noop.call(&mut store, ());
            })
        })
    });
}

pub fn native_echo_noop(c: &mut Criterion) {
    let mut group = c.benchmark_group("native-echo");
    group.measurement_time(Duration::from_secs(10));

    fn noop() {}

    group.bench_function("noop", |b| b.iter(|| black_box(noop())));
}

criterion_group!(benches, wasm_echo_noop, native_echo_noop, bmvm_echo_noop);
criterion_main!(benches);
