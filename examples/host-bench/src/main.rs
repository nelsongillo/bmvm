use bmvm_host::mem::{alloc_buf, AlignedUsize, Foreign, ForeignBuf, SharedBuf};
use bmvm_host::{Buffer, ConfigBuilder as VmConfigBuilder, ModuleBuilder, expose, linker};
use clap::Parser;
use std::path::PathBuf;

const ENV_GUEST: &str = "GUEST";
const ENV_DEBUG: &str = "DEBUG";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = ENV_GUEST)]
    guest: String,

    #[arg(short, long, env = ENV_DEBUG, default_value_t = false)]
    debug: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // logging
    let mut log_builder = env_logger::Builder::from_default_env();
    match args.debug {
        true => log_builder.filter_level(log::LevelFilter::Debug),
        false => log_builder.filter_level(log::LevelFilter::Info),
    }
    .init();

    let path = PathBuf::from(args.guest);

    let linker = linker::ConfigBuilder::new()
        .register_guest_function::<(SharedBuf, SharedBuf, SharedBuf), ForeignBuf>("encrypt")
        .register_guest_function::<(SharedBuf, SharedBuf, SharedBuf), ForeignBuf>("decrypt");

    let builder = ModuleBuilder::new()
        .configure_linker(linker)
        .with_path(path.as_path());

    let mut module = builder.build()?;

    let fn_encode = module.get_upcall::<(SharedBuf, SharedBuf, SharedBuf), ForeignBuf>("encrypt").unwrap();
    let fn_decode = module.get_upcall::<(SharedBuf, SharedBuf, SharedBuf), ForeignBuf>("decrypt").unwrap();

    let mut encode = unsafe { alloc_buf("hello, world".len() + 16).ok().unwrap() };
    let key1 = unsafe { alloc_buf(256/8).ok().unwrap() }.into_shared();
    let nonce1 = unsafe { alloc_buf(96/8).ok().unwrap() }.into_shared();

    let key2 = unsafe { alloc_buf(256/8).ok().unwrap() }.into_shared();
    let nonce2 = unsafe { alloc_buf(96/8).ok().unwrap() }.into_shared();

    encode.as_mut().copy_from_slice(b"hello, world");

    let start = std::time::Instant::now();

    let encoded = fn_encode.call(&mut module,( key1, encode.into_shared(),nonce1)).unwrap();
    println!("encoded: {}", String::from_utf8_lossy(encoded.as_ref()));
    let decoded = fn_decode.call(&mut module,( key2, encoded.owned().into_shared(),nonce2)).unwrap();
    println!("decoded: {}", String::from_utf8_lossy(decoded.as_ref()));

    let elapsed = start.elapsed();
    println!("{elapsed:?}");

    Ok(())
}
