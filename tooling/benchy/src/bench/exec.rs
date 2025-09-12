use bmvm_host::mem::AlignedUsize;
use bmvm_host::{ConfigBuilder, Module as BmvmModule, ModuleBuilder, Upcall, linker};
use indicatif::ProgressBar;
use std::path::PathBuf;
use std::process::Output;
use std::time::Instant;
use wasmtime::{Engine, Instance, Module as WasmModule, Store, TypedFunc};

type Pre<T> = fn(&PathBuf) -> anyhow::Result<T>;
type Exec<T> = fn(&mut T) -> anyhow::Result<f64>;
type Post<T> = fn(&mut T) -> anyhow::Result<()>;

pub fn native(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<PathBuf> {
        Ok(path.clone())
    }
    fn exec(path: &mut PathBuf) -> anyhow::Result<f64> {
        let output: Output = std::process::Command::new(&path).output()?;
        let s = String::from_utf8_lossy(&output.stdout);
        let v = s.parse::<u64>()?;
        Ok(v as f64)
    }
    fn post(_: &mut PathBuf) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
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
        run.call(store, ())?;
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
            .configure_vm(ConfigBuilder::new().shared_memory(AlignedUsize::new_aligned(0).unwrap()))
            .configure_linker(linker::ConfigBuilder::new().register_guest_function::<(), ()>("run"))
            .with_path(path)
            .build()?;

        let run = module.get_upcall::<(), ()>("run")?;
        Ok((run, module))
    }
    fn exec((run, guest): &mut (Upcall<(), ()>, BmvmModule)) -> anyhow::Result<f64> {
        let now = Instant::now();
        run.call(guest, ())?;
        let elapsed = now.elapsed();
        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut (Upcall<(), ()>, BmvmModule)) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}

fn bench<T>(
    path: &PathBuf,
    warmup: usize,
    iters: usize,
    prep: Pre<T>,
    exec: Exec<T>,
    post: Post<T>,
) -> anyhow::Result<Vec<f64>> {
    let mut samples: Vec<f64> = Vec::with_capacity(iters);
    println!("Executable: {}", path.display());

    let mut state = prep(&path)?;

    // Executing optional warmup phase
    if warmup > 0 {
        println!("Warmup...");
        let bar = ProgressBar::new(warmup as u64);
        bar.set_position(0);
        for i in 0..warmup {
            let _ = exec(&mut state)?;
            bar.inc(i as u64 + 1);
        }
        bar.finish();
    }

    // Executing Sampling
    println!("Sampling...");
    let bar = ProgressBar::new(iters as u64);
    bar.set_position(0);
    for i in 0..iters {
        let sample = exec(&mut state)?;
        samples.push(sample);
        bar.set_position(i as u64 + 1);
    }
    bar.finish();
    println!("Execution Finished.");

    post(&mut state)?;

    Ok(samples)
}
