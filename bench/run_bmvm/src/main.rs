use bmvm_host::{ConfigBuilder, ModuleBuilder, linker, mem::AlignedUsize};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let mut module = ModuleBuilder::new()
        .configure_vm(ConfigBuilder::new().shared_memory(AlignedUsize::zero()))
        .configure_linker(linker::ConfigBuilder::new().register_guest_function::<(), ()>("run"))
        .with_path(path.as_ref())
        .build()?;

    let run = module.get_upcall::<(), ()>("run")?;
    run.call(&mut module, ())?;
    Ok(())
}
