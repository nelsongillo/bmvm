use bmvm_host::{ConfigBuilder, Runtime};
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // logging
    let mut log_builder = env_logger::Builder::from_default_env();
    if args.debug {
        log_builder.filter_level(log::LevelFilter::Debug);
    } else {
        log_builder.filter_level(log::LevelFilter::Info);
    }
    log_builder.init();

    // configuration
    let cfg = ConfigBuilder::new().debug(args.debug).build();
    let mut runtime = Runtime::new(cfg, args.guest)?;
    match runtime.run() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("{:?}", e)),
    }
}
