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
#[derive(Debug)]
pub struct Module {
    vm: vm::Vm,
}

impl Module {
    fn new(vm: vm::Config, linker: linker::Config, buf: &Buffer) -> Result<Module> {
        let mut vm = vm::Vm::new(vm)?;
        let mut linker = linker::Linker::new(linker)?;
        // parse the guest executable
        let mut executable = ExecBundle::from_buffer(buf, vm.allocator())?;

        // execute linking stage
        linker.link(&executable)?;

        vm.load_exec(&mut executable)?;
        let (upcalls, hypercalls) = linker.into_calls();

        vm.link(hypercalls, upcalls);
        vm.run().map_err(Error::Vm)?;
        Ok(Self { vm })
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
        self.vm.run()?;
        self.vm.upcall_result::<R>().map_err(Error::Upcall)
    }
}

pub struct ModuleBuilder<'a> {
    vm: vm::Config,
    linker: linker::Config,
    path: Option<&'a Path>,
    buffer: Option<&'a Buffer>,
}

impl<'a> Default for ModuleBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ModuleBuilder<'a> {
    pub fn new() -> Self {
        Self {
            vm: vm::Config::default(),
            linker: linker::Config::default(),
            path: None,
            buffer: None,
        }
    }

    pub fn configure_vm<C: Into<vm::Config>>(mut self, config: C) -> Self {
        self.vm = config.into();
        self
    }

    pub fn configure_linker<C: Into<linker::Config>>(mut self, config: C) -> Self {
        self.linker = config.into();
        self
    }

    /// Load the executable from a path.
    /// Note: Any previously set buffer will be ignored.
    pub fn with_path(mut self, path: &'a Path) -> Self {
        self.path = Some(path);
        self.buffer = None;
        self
    }

    /// Load the executable from a buffer.
    /// Note: Any previously set path will be ignored.
    pub fn with_buffer(mut self, buffer: &'a Buffer) -> Self {
        self.buffer = Some(buffer);
        self.path = None;
        self
    }

    pub fn build(self) -> Result<Module> {
        if self.path.is_none() && self.buffer.is_none() {
            return Err(Error::MissingExecutable);
        }

        if let Some(buf) = self.buffer {
            Module::new(self.vm, self.linker, buf)
        } else {
            let buf = Buffer::new(self.path.unwrap())?;
            Module::new(self.vm, self.linker, &buf)
        }
    }
}
