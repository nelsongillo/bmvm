use bmvm_host::{ModuleBuilder, linker};
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

    // configuration
    let linker = linker::ConfigBuilder::new()
        .register_guest_function::<(), ()>("noop")
        .build();

    let path = PathBuf::from(args.guest);
    let mut module = ModuleBuilder::new()
        .with_path(&path)
        .configure_linker(linker)
        .build()?;

    let noop = module.get_upcall::<(), ()>("noop")?;

    let now = std::time::Instant::now();

    for i in 0..2_000_000 {
        noop.call(&mut module, ())?;
    }

    println!("DONE IN {:?}", now.elapsed());
    Ok(())
}
