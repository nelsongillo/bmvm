use crate::bench::bench;
use bmvm_host::mem::{AlignedNonZeroUsize, AlignedUsize};
use bmvm_host::{ConfigBuilder, ModuleBuilder, linker};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;
use wasmtime::{Engine, Instance, Module as WasmModule, Store};

pub fn wasm(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<PathBuf> {
        Ok(path.clone())
    }
    fn exec(path: &mut PathBuf) -> anyhow::Result<f64> {
        let now = Instant::now();

        let instance = black_box({
            let buf = std::fs::read(path)?;

            let engine = Engine::default();
            let module = WasmModule::from_binary(&engine, buf.as_slice())?;
            let mut store = Store::new(&engine, ());
            Instance::new(&mut store, &module, &[])?
        });

        let elapsed = now.elapsed();
        std::mem::drop(instance);

        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut PathBuf) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}

pub fn bmvm(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<PathBuf> {
        Ok(path.clone())
    }
    fn exec(path: &mut PathBuf) -> anyhow::Result<f64> {
        let stack = AlignedNonZeroUsize::new_ceil(1).unwrap();
        let now = Instant::now();

        let module = black_box({
            ModuleBuilder::new()
                .configure_vm(
                    ConfigBuilder::new()
                        .stack_size(stack)
                        .shared_memory(AlignedUsize::zero()),
                )
                .configure_linker(linker::ConfigBuilder::new())
                .with_path(path)
                .build()?
        });

        let elapsed = now.elapsed();
        std::mem::drop(module);

        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut PathBuf) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}
