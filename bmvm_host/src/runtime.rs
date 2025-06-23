use crate::linker::{Linker, LinkerConfig};
use crate::vm;
use crate::{
    elf,
    elf::{Buffer, ExecBundle},
};
use std::path::Path;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("vm error: {0}")]
    Vm(#[from] vm::Error),
    #[error("elf error: {0}")]
    Elf(#[from] elf::Error),
}

pub struct Runtime {
    vm: vm::Vm,
    linker: Linker,
    executable: ExecBundle,
}

impl Runtime {
    pub fn new<C, P>(cfg: C, path: P) -> Result<Self>
    where
        C: Into<vm::Config>,
        P: AsRef<Path>,
    {
        let buffer = Buffer::new(path)?;

        let vm = vm::Vm::new(cfg)?;
        let linker = Linker::new(LinkerConfig::default());
        let executable = ExecBundle::from_buffer(buffer, vm.allocatort())?;

        Ok(Self {
            vm,
            linker,
            executable,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.vm.load_exec(&mut self.executable)?;
        self.vm.run().map_err(Error::Vm)
    }
}
