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
        self.vm.run()?;
        self.vm.upcall_result::<R>().map_err(Error::Upcall)
    }
}

pub struct Runtime {
    vm: vm::Vm,
    linker: linker::Linker,
    executable: ExecBundle,
}

impl Runtime {
    fn new(vm: vm::Config, linker: linker::Config, buf: &Buffer) -> Result<Self> {
        let vm = vm::Vm::new(vm)?;
        let mut linker = linker::Linker::new(linker)?;
        // parse the guest executable
        let executable = ExecBundle::from_buffer(buf, vm.allocator())?;

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
        self.vm.run().map_err(Error::Vm)?;
        Ok(Module { vm: self.vm })
    }
}

pub struct RuntimeBuilder<'a> {
    vm: vm::Config,
    linker: linker::Config,
    path: Option<&'a Path>,
    buffer: Option<&'a Buffer>,
}

impl<'a> Default for RuntimeBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> RuntimeBuilder<'a> {
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

    pub fn build(self) -> Result<Runtime> {
        if self.path.is_none() && self.buffer.is_none() {
            return Err(Error::MissingExecutable);
        }

        if let Some(buf) = self.buffer {
            Runtime::new(self.vm, self.linker, buf)
        } else {
            let buf = Buffer::new(self.path.unwrap())?;
            Runtime::new(self.vm, self.linker, &buf)
        }
    }
}
