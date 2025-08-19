use bmvm_host::{ConfigBuilder, RuntimeBuilder, linker};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("expected path to guest executable");

    // configuration
    let cfg = ConfigBuilder::new();
    let linker = linker::ConfigBuilder::default();

    let runtime = RuntimeBuilder::new()
        .linker(linker)
        .vm(cfg)
        .executable(path)
        .build()?;
    let _ = runtime.setup()?;

    Ok(())
}
