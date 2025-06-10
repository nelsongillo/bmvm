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

pub struct Config {
    stack_size: u64,
    max_memory: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stack_size: 2 * 1024 * 1024,   // 2MiB
            max_memory: 128 * 1024 * 1024, // 128MiB
        }
    }
}

pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        ConfigBuilder {
            config: Config::default(),
        }
    }

    pub fn stack_size(mut self, size: u64) -> Self {
        self.config.stack_size = size;
        self
    }

    pub fn max_memory(mut self, size: u64) -> Self {
        self.config.max_memory = size;
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

impl From<ConfigBuilder> for Config {
    fn from(builder: ConfigBuilder) -> Self {
        builder.build()
    }
}

pub struct Runtime {
    config: Config,
    vm: vm::Vm,
    linker: Linker,
    executable: ExecBundle,
}

impl Runtime {
    pub fn new<C, P>(cfg: C, path: P) -> Result<Self>
    where
        C: Into<Config>,
        P: AsRef<Path>,
    {
        let buffer = Buffer::new(path)?;

        let vm = vm::Vm::new(vm::Config::default())?;
        let linker = Linker::new(LinkerConfig::default());
        let executable = ExecBundle::from_buffer(buffer, vm.allocatort())?;

        Ok(Self {
            config: cfg.into(),
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
