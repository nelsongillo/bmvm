use crate::{
    elf,
    elf::{Buffer, ExecBundle},
};
use crate::{linker, vm};
use bmvm_common::registry::Params;
use bmvm_common::vmi::ForeignShareable;
use std::path::Path;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No executable provided")]
    MissingExecutable,
    #[error("upcall error: {0}")]
    Upcall(vm::Error),
    #[error("linker error: {0}")]
    Linker(#[from] linker::Error),
    #[error("vm error: {0}")]
    Vm(#[from] vm::Error),
    #[error("elf error: {0}")]
    Elf(#[from] elf::Error),
}

/// A module is a loaded and initialized guest executable on which the host can call functions.
pub struct Module {
    vm: vm::Vm,
}

impl Module {
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

    /// Setup the module by loading the proxy OS and executing the module setup code.
    /// Returns a initialized Module on which the host can call functions
    pub fn setup(mut self) -> Result<Module> {
        self.vm.load_exec(&mut self.executable)?;
        let (upcalls, hypercalls) = self.linker.into_calls();

        self.vm.link(hypercalls, upcalls);
        self.vm.run::<()>(false).map_err(Error::Vm)?;
        Ok(Module { vm: self.vm })
    }
}

pub struct RuntimeBuilder<P: AsRef<Path>> {
    vm: vm::Config,
    linker: linker::Config,
    path: Option<P>,
}

impl<P: AsRef<Path>> RuntimeBuilder<P> {
    pub fn new() -> Self {
        Self {
            vm: vm::Config::default(),
            linker: linker::Config::default(),
            path: None,
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

    pub fn executable(mut self, path: P) -> Self {
        self.path = Some(path);
        self
    }

    pub fn build(self) -> Result<Runtime> {
        if self.path.is_none() {
            return Err(Error::MissingExecutable);
        }

        Runtime::new(self.vm, self.linker, self.path.unwrap())
    }
}
