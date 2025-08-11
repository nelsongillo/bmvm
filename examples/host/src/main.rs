use bmvm_host::{
    ConfigBuilder, Foreign, ForeignBuf, RuntimeBuilder, Shared, SharedBuf, TypeSignature, alloc,
    expose, linker,
};
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

#[repr(transparent)]
#[derive(TypeSignature)]
struct Foo(Bar);

#[repr(C)]
#[derive(TypeSignature)]
struct Bar {
    a: u32,
    b: u32,
}

#[expose]
extern "C" fn x(_a: Foo, _b: i32) -> Shared<Bar> {
    let mut owned = unsafe { alloc::<Bar>().unwrap() };
    let bar = owned.as_mut();
    bar.a = 13;
    bar.b = 12;

    owned.into_shared()
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
    let cfg = ConfigBuilder::new().debug(args.debug);

    let mut runtime = RuntimeBuilder::new()
        .linker(linker::ConfigBuilder::new())
        .vm(cfg)
        .executable(args.guest)
        .build()?;
    match runtime.setup() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("{:?}", e)),
    }
}
