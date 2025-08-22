use bmvm_host::{ConfigBuilder, ForeignBuf, RuntimeBuilder, expose, linker};
use clap::Parser;

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

const FUNC_HYPERCALL_REDIRECT: &str = "hypercall_redirect";

#[expose]
pub fn add(a: u64, b: u64) -> u64 {
    let result = a + b;
    result
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
    let cfg = ConfigBuilder::new().debug(args.debug);
    let linker =
        linker::ConfigBuilder::new().register_guest_function::<(), u64>(FUNC_HYPERCALL_REDIRECT);

    let runtime = RuntimeBuilder::new()
        .linker(linker)
        .vm(cfg)
        .executable(args.guest)
        .build()?;
    let mut module = runtime.setup()?;

    let expect = 30;
    let result = module.call::<(), u64>(FUNC_HYPERCALL_REDIRECT, ())?;
    log::info!("DONE: {FUNC_HYPERCALL_REDIRECT}");
    assert_eq!(result, expect);
    Ok(())
}
