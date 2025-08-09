use crate::{
    elf,
    elf::{Buffer, ExecBundle},
};
use crate::{linker, vm};
use bmvm_common::registry::Params;
use bmvm_common::vmi::ForeignShareable;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("upcall error: {0}")]
    Upcall(vm::Error),
    #[error("linker error: {0}")]
    Linker(#[from] linker::Error),
    #[error("vm error: {0}")]
    Vm(#[from] vm::Error),
    #[error("elf error: {0}")]
    Elf(#[from] elf::Error),
}

pub struct Runtime {
    vm: vm::Vm,
    linker: linker::Linker,
    executable: ExecBundle,
}

impl Runtime {
    fn new<E>(vm: vm::Config, linker: linker::Config, executable: E) -> Result<Self>
    where
        E: AsRef<Path>,
    {
        let buffer = Buffer::new(executable)?;

        let vm = vm::Vm::new(vm)?;
        let mut linker = linker::Linker::new(linker)?;
        // parse the guest executable
        let executable = ExecBundle::from_buffer(buffer, vm.allocator())?;

        // execute linking stage
        linker.link(&executable)?;

        Ok(Self {
            vm,
            linker,
            executable,
        })
    }

    /// Setup the guest by loading the proxy OS and executing the guest setup code.
    pub fn setup(&mut self) -> Result<()> {
        self.vm.load_exec(&mut self.executable)?;
        self.vm.run::<()>().map_err(Error::Vm)
    }

    /// Try calling a function on the guest with the provided parameters.
    /// Error if the function is not found or the signatures do not match.
    pub fn call<P, R>(&mut self, func: &'static str, params: P) -> Result<R>
    where
        P: Params,
        R: ForeignShareable,
    {
        self.vm
            .upcall_exec_setup::<P, R>(func, params)
            .map_err(Error::Upcall)?;
        self.vm.run::<R>()?;
        self.vm.upcall_result::<R>().map_err(Error::Upcall)
    }
}

pub struct RuntimeBuilder {
    vm: vm::Config,
    linker: linker::Config,
    path: PathBuf,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            vm: vm::Config::default(),
            linker: linker::Config::default(),
            path: PathBuf::new(),
        }
    }

    pub fn vm<C: Into<vm::Config>>(mut self, config: C) -> Self {
        self.vm = config.into();
        self
    }

    pub fn linker<C: Into<linker::Config>>(mut self, config: C) -> Self {
        self.linker = config.into();
        self
    }

    pub fn executable(mut self, path: impl AsRef<Path>) -> Self {
        self.path = path.as_ref().to_path_buf();
        self
    }

    pub fn build(self) -> Result<Runtime> {
        Runtime::new(self.vm, self.linker, self.path)
    }
}
