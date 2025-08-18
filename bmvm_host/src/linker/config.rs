use crate::linker::upcall;
use bmvm_common::registry::Params;
use bmvm_common::vmi::ForeignShareable;

const ERR_ON_UNUSED_HOST: bool = false;
const ERR_ON_UNUSED_GUEST: bool = false;

#[derive(Debug)]
pub struct Config {
    pub(super) error_unused_host: bool,
    pub(super) error_unused_guest: bool,
    pub(super) upcalls: Vec<upcall::Function>,
}

impl From<ConfigBuilder> for Config {
    fn from(builder: ConfigBuilder) -> Self {
        builder.build()
    }
}

impl Default for Config {
    fn default() -> Self {
        ConfigBuilder::default().build()
    }
}

pub struct ConfigBuilder {
    config: Config,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    /// Create a new ConfigBuilder
    pub fn new() -> Self {
        Self {
            config: Config {
                error_unused_host: ERR_ON_UNUSED_HOST,
                error_unused_guest: ERR_ON_UNUSED_GUEST,
                upcalls: Vec::new(),
            },
        }
    }

    pub fn error_unused_host(mut self, error: bool) -> Self {
        self.config.error_unused_host = error;
        self
    }

    pub fn error_unused_guest(mut self, error: bool) -> Self {
        self.config.error_unused_guest = error;
        self
    }

    /// Register a function on the guest, which will be called by the host.
    pub fn register_guest_function<P, R>(mut self, name: &'static str) -> Self
    where
        P: Params,
        R: ForeignShareable,
    {
        let func = upcall::Function::new::<P, R>(name);
        self.config.upcalls.push(func);
        self
    }

    /// Build the final configuration.
    pub fn build(self) -> Config {
        self.config
    }
}
