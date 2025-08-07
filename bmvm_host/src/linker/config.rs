use crate::linker::upcall::Function;
use bmvm_common::mem::ForeignShareable;
use bmvm_common::registry::Params;

const ERR_ON_UNUSED_HOST: bool = false;
const ERR_ON_UNUSED_GUEST: bool = false;

#[derive(Debug)]
pub struct Config {
    pub(crate) error_unused_host: bool,
    pub(crate) error_unused_guest: bool,
    pub(crate) expected_functions: Vec<Function>,
}

impl Default for Config {
    fn default() -> Self {
        ConfigBuilder::default().build()
    }
}

struct ConfigBuilder {
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
                expected_functions: vec![],
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

    pub fn register_guest_function<P, R>(mut self, name: &'static str) -> Self
    where
        P: Params,
        R: ForeignShareable,
    {
        let func = Function::new::<P, R>(name);
        self.config.expected_functions.push(func);
        self
    }

    /// Build the final configuration.
    pub fn build(self) -> Config {
        self.config
    }
}
