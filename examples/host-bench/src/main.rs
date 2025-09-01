use bmvm_host::mem::AlignedUsize;
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

    let shared_memory = AlignedUsize::new_unchecked(0);
    for _ in 0..1000 {
        let builder = ModuleBuilder::new()
            .configure_vm(VmConfigBuilder::new().shared_memory(shared_memory))
            .with_path(path.as_path());

        let start = std::time::Instant::now();
        let mut module = builder.build()?;
        let elapsed = start.elapsed();
        println!("{elapsed:?}");

        eprint!("{module:?}")
    }

    Ok(())
}
