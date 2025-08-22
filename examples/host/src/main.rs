use bmvm_host::{
    ConfigBuilder, ForeignBuf, RuntimeBuilder, Shared, SharedBuf, TypeSignature, alloc, alloc_buf,
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

const FUNC_FOO: &str = "foo";
const FUNC_SUM: &str = "sum";
type FooParams = (u32, Shared<Foo>);

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
    let linker = linker::ConfigBuilder::new()
        .register_guest_function::<FooParams, u32>(FUNC_FOO)
        .register_guest_function::<(SharedBuf,), u64>(FUNC_SUM);

    let runtime = RuntimeBuilder::new()
        .linker(linker)
        .vm(cfg)
        .executable(args.guest)
        .build()?;
    let mut module = runtime.setup()?;

    let mut owned_foo = unsafe { alloc::<Foo>()? };
    let foo = owned_foo.as_mut();
    foo.0.a = 0xF0;
    foo.0.b = 0xF00;
    let shared_foo = owned_foo.into_shared();

    let sum = module.call::<FooParams, u32>(FUNC_FOO, (0xF, shared_foo))?;
    log::info!("Foo successfull: {sum:X}");

    let mut sum_buf = unsafe { alloc_buf(0x100)? };
    let mut buf = sum_buf.as_mut();
    for i in 0..0x100 {
        buf[i] = i as u8;
    }

    let expect = (0..0x100).fold(0, |acc, x| acc + x as u64);

    let another = module.call::<(SharedBuf,), u64>(FUNC_SUM, (sum_buf.into_shared(),))?;
    log::info!("Sum successfull: Got {another:X} expected {expect:X}");
    Ok(())
}
