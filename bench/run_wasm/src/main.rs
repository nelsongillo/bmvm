use wasmtime::{Engine, Instance, Module as WasmModule, Store};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let buf = std::fs::read(path)?;
    let engine = Engine::default();
    let module = WasmModule::from_binary(&engine, buf.as_slice())?;
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[])?;
    let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;

    run.call(store, ())?;
    Ok(())
}
