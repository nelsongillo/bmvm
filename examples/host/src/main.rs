use bmvm_host::mem::{AlignedNonZeroUsize, ForeignBuf, SharedBuf, alloc_buf};
use bmvm_host::{ConfigBuilder, ModuleBuilder, linker};
use clap::Parser;
use std::hint::black_box;
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

    // configuration
    let linker = linker::ConfigBuilder::new()
        .register_guest_function::<(), ()>("noop")
        .register_guest_function::<(SharedBuf,), ForeignBuf>("reverse")
        .build();

    let vm = ConfigBuilder::new()
        .debug(args.debug)
        .stack_size(AlignedNonZeroUsize::new_ceil(BMVM_STACK).unwrap());

    const BMVM_STACK: usize = 32 * 1024 * 1024; // 32MiB
    let path = PathBuf::from(args.guest);
    let mut module = ModuleBuilder::new()
        .with_path(&path)
        .configure_linker(linker)
        .configure_vm(vm)
        .build()?;

    let reverse = module
        .get_upcall::<(SharedBuf,), ForeignBuf>("reverse")
        .unwrap();

    let now = std::time::Instant::now();
    for _ in 0..2_000_000 {
        let owned = unsafe { alloc_buf(1024)? };
        let _ =  reverse.call(&mut module, (owned.into_shared(),)).unwrap();
    }

    println!("DONE IN {:?}", now.elapsed());
    Ok(())
}
