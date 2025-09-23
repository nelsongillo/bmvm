use crate::bench::bench;
use bmvm_host::mem::{AlignedNonZeroUsize, AlignedUsize};
use bmvm_host::{ConfigBuilder, ModuleBuilder, Upcall, expose, linker};
use const_format::formatcp;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;
use wasmtime::{Engine, Instance, Linker, Module as WasmModule, Store, TypedFunc};

#[cfg(feature = "links1")]
const LINKS: usize = 1;
#[cfg(feature = "links8")]
const LINKS: usize = 8;
#[cfg(feature = "links16")]
const LINKS: usize = 16;
#[cfg(feature = "links32")]
const LINKS: usize = 32;
#[cfg(feature = "links64")]
const LINKS: usize = 64;
#[cfg(feature = "links128")]
const LINKS: usize = 128;

pub fn wasm(path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
    fn pre(path: &PathBuf) -> anyhow::Result<PathBuf> {
        Ok(path.clone())
    }
    fn exec(path: &mut PathBuf) -> anyhow::Result<f64> {
        let mut calls: Vec<TypedFunc<(), i32>> = Vec::with_capacity(LINKS);

        let now = Instant::now();

        let instance = black_box({
            let buf = std::fs::read(path)?;

            // Create the Wasmtime engine and store
            let engine = Engine::default();
            let mut store = Store::new(&engine, ());
            let module = WasmModule::from_binary(&engine, buf.as_slice())?;
            let mut linker = Linker::new(&engine);

            #[cfg(feature = "links1")]
            linker.func_wrap("env", "hyper0", || 0i32);

            #[cfg(feature = "links8")]
            loop_code::repeat!(INDEX 8 {
                linker.func_wrap("env", format!("hyper{}", INDEX).as_str(), || 0i32);
            });

            #[cfg(feature = "links16")]
            loop_code::repeat!(INDEX 16 {
                linker.func_wrap("env", format!("hyper{}", INDEX).as_str(), || 0i32);
            });

            #[cfg(feature = "links32")]
            loop_code::repeat!(INDEX 32 {
                linker.func_wrap("env", format!("hyper{}", INDEX).as_str(), || 0i32);
            });

            #[cfg(feature = "links64")]
            loop_code::repeat!(INDEX 64 {
                linker.func_wrap("env", format!("hyper{}", INDEX).as_str(), || 0i32);
            });

            #[cfg(feature = "links128")]
            loop_code::repeat!(INDEX 128 {
                linker.func_wrap("env", format!("hyper{}", INDEX).as_str(), || 0i32);
            });

            let instance: Instance = linker.instantiate(&mut store, &module)?;

            #[cfg(feature = "links1")]
            {
                calls.push(instance.get_typed_func::<(), i32>(&mut store, "up0")?);
            }

            #[cfg(feature = "links8")]
            loop_code::repeat!(INDEX 8 {
                calls.push(instance.get_typed_func::<(), i32>(&mut store, formatcp!("up{}", INDEX))?);
            });

            #[cfg(feature = "links16")]
            loop_code::repeat!(INDEX 16 {
               calls.push(instance.get_typed_func::<(), i32>(&mut store, formatcp!("up{}", INDEX))?);
            });

            #[cfg(feature = "links32")]
            loop_code::repeat!(INDEX 32 {
               calls.push(instance.get_typed_func::<(), i32>(&mut store, formatcp!("up{}", INDEX))?);
            });

            #[cfg(feature = "links64")]
            loop_code::repeat!(INDEX 64 {
               calls.push(instance.get_typed_func::<(), i32>(&mut store, formatcp!("up{}", INDEX))?);
            });

            #[cfg(feature = "links128")]
            loop_code::repeat!(INDEX 128 {
                calls.push(instance.get_typed_func::<(), i32>(&mut store, formatcp!("up{}", INDEX))?);
            });
        });
        let elapsed = now.elapsed();
        std::mem::drop(calls);
        let _ = instance;

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
        let mut calls: Vec<Upcall<(), i32>> = Vec::with_capacity(LINKS);
        let now = Instant::now();

        let mut linker = linker::ConfigBuilder::new();
        #[cfg(feature = "links1")]
        {
            linker = linker.register_guest_function::<(), i32>("up0");
        }

        #[cfg(feature = "links8")]
        loop_code::repeat!(INDEX 8 {
            linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
        });

        #[cfg(feature = "links16")]
        loop_code::repeat!(INDEX 16 {
                linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
        });

        #[cfg(feature = "links32")]
        loop_code::repeat!(INDEX 32 {
            linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
        });

        #[cfg(feature = "links64")]
        loop_code::repeat!(INDEX 64 {
            linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
        });

        #[cfg(feature = "links128")]
        loop_code::repeat!(INDEX 128 {
            linker = linker.register_guest_function::<(), i32>(formatcp!("up{}", INDEX));
        });

        let mut module = black_box({
            ModuleBuilder::new()
                .configure_vm(
                    ConfigBuilder::new()
                        .stack_size(stack)
                        .shared_memory(AlignedUsize::zero()),
                )
                .configure_linker(linker)
                .with_path(path)
                .build()?
        });

        #[cfg(feature = "links1")]
        {
            calls.push(module.get_upcall::<(), i32>("up0")?);
        }

        #[cfg(feature = "links8")]
        loop_code::repeat!(INDEX 8 {
            calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
        });

        #[cfg(feature = "links16")]
        loop_code::repeat!(INDEX 16 {
            calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
        });

        #[cfg(feature = "links32")]
        loop_code::repeat!(INDEX 32 {
                calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
        });

        #[cfg(feature = "links64")]
        loop_code::repeat!(INDEX 64 {
            calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
        });

        #[cfg(feature = "links128")]
        loop_code::repeat!(INDEX 128 {
            calls.push(module.get_upcall::<(), i32>(formatcp!("up{}", INDEX))?);
        });

        let elapsed = now.elapsed();
        std::mem::drop(module);
        std::mem::drop(calls);

        Ok(elapsed.as_nanos() as f64)
    }
    fn post(_: &mut PathBuf) -> anyhow::Result<()> {
        Ok(())
    }
    bench(path, warmup, iters, pre, exec, post)
}

#[cfg(feature = "links1")]
seq_macro::seq!(N in 0..1 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }

    static UP~N: &'static str = "up~N";
});

#[cfg(feature = "links8")]
seq_macro::seq!(N in 0..8 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links16")]
seq_macro::seq!(N in 0..16 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links32")]
seq_macro::seq!(N in 0..32 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links64")]
seq_macro::seq!(N in 0..64 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});

#[cfg(feature = "links128")]
seq_macro::seq!(N in 0..128 {
    #[expose]
    pub extern "C" fn hyper~N() -> i32 {
        N
    }
});
