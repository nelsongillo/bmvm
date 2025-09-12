use std::hint::black_box;
use wasmtime::{Engine, Instance, Module as WasmModule, Store};

const WASM: &str = "/home/nelson/TUM/master/kvm/bench/binaries/wasm-echo.wasm";

fn main() {
    let buf = std::fs::read(WASM).unwrap();

    let engine = Engine::default();
    let module = WasmModule::from_binary(&engine, buf.as_slice()).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();

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

    let mut buf = [0u8; 64];
    (0..1000000).for_each(|i| {
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
            println!("{i}")
        })
    })
}
