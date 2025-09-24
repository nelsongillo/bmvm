use crate::bench::bench;
use bmvm_host::mem::AlignedUsize;
use bmvm_host::{ConfigBuilder, Module as BmvmModule, ModuleBuilder, Upcall, linker};
use std::ffi::OsStr;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Output;
use std::time::Instant;
use wasmtime::{Engine, Instance, Module as WasmModule, Store, TypedFunc};

pub fn native(
    path: &PathBuf,
    warmup: usize,
    iters: usize,
    args: String,
) -> anyhow::Result<Vec<f64>> {
    fn pre((path, args): (&PathBuf, String)) -> anyhow::Result<(PathBuf, String)> {
        Ok((path.clone(), args))
    }
    fn exec((path, args): &mut (PathBuf, String)) -> anyhow::Result<f64> {
        let output: Output = std::process::Command::new(&path)
            .args(args.split_ascii_whitespace())
            .output()?;
        let s = String::from_utf8_lossy(&output.stdout);
        let v = s.parse::<u64>()?;
        Ok(v as f64)
    }
    fn post(_: &mut (PathBuf, String)) -> anyhow::Result<()> {
        Ok(())
    }
    bench((path, args), warmup, iters, pre, exec, post)
}

pub fn wasm(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<(TypedFunc<(), ()>, Store<()>)> {
        let buf = std::fs::read(path)?;

        let engine = Engine::default();
        let module = WasmModule::from_binary(&engine, buf.as_slice())?;
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;
        let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

        Ok((run, store))
    }
    fn exec((run, store): &mut (TypedFunc<(), ()>, Store<()>)) -> anyhow::Result<f64> {
        let now = Instant::now();
        black_box(run.call(store, ())?);
        let elapsed = now.elapsed();
        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut (TypedFunc<(), ()>, Store<()>)) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}

pub fn bmvm(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<(Upcall<(), ()>, BmvmModule)> {
        let mut module = ModuleBuilder::new()
            .configure_vm(ConfigBuilder::new().shared_memory(AlignedUsize::zero()))
            .configure_linker(linker::ConfigBuilder::new().register_guest_function::<(), ()>("run"))
            .with_path(path)
            .build()?;

        let run = module.get_upcall::<(), ()>("run")?;
        Ok((run, module))
    }
    fn exec((run, guest): &mut (Upcall<(), ()>, BmvmModule)) -> anyhow::Result<f64> {
        let now = Instant::now();
        black_box({
            run.call(guest, ())?;
        });
        let elapsed = now.elapsed();
        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut (Upcall<(), ()>, BmvmModule)) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}
