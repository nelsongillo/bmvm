use bmvm_host::{Config, Runtime};
use std::env::args;

// TODO: Why trigger KVM_EXIT_SHUTDOWN on entry?! probably
fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = args().collect::<Vec<String>>();
    if args.len() < 2 {
        log::error!("Usage: {} <executable>", args[0]);
        return Ok(());
    }

    let mut runtime = Runtime::new(Config::default(), args[1].clone())?;
    match runtime.run() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("{:?}", e)),
    }
}
