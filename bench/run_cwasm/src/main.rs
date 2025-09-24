use wasmtime::{Engine, Instance, Module as WasmModule, Store};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let engine = Engine::default();
    let module = unsafe { WasmModule::deserialize_file(&engine, path) }?;
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[])?;
    let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

    run.call(store, ())?;
    Ok(())
}
