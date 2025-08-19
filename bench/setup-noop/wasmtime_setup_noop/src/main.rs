use std::error::Error;
use wasmtime::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).expect("expected path to wasm file");

    let engine = Engine::default();
    let module: Module;
    #[cfg(any(not(feature = "bin"), feature = "wat"))]
    {
        module = Module::from_file(&engine, path)?;
    }
    #[cfg(feature = "bin")]
    unsafe {
        let contents = std::fs::read(path)?;
        module = Module::deserialize(&engine, std::hint::black_box(contents))?;
    }
    let mut store = Store::new(&engine, ());
    let _ = Instance::new(&mut store, &module, &[])?;
    Ok(())
}
