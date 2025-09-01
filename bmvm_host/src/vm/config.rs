use crate::{DEFAULT_SHARED_MEMORY, GUEST_DEFAULT_STACK_SIZE};
use bmvm_common::mem::{AlignedNonZeroUsize, AlignedUsize};

#[derive(Debug)]
pub struct Config {
    pub(crate) stack_size: AlignedNonZeroUsize,
    pub(crate) shared_memory: AlignedUsize,
    pub(crate) debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stack_size: AlignedNonZeroUsize::new_ceil(GUEST_DEFAULT_STACK_SIZE).unwrap(),
            shared_memory: AlignedUsize::new_ceil(DEFAULT_SHARED_MEMORY),
            debug: false,
        }
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
    pub fn new() -> Self {
        ConfigBuilder {
            config: Config::default(),
        }
    }

    pub fn stack_size(mut self, size: AlignedNonZeroUsize) -> Self {
        self.config.stack_size = size;
        self
    }

    pub fn shared_memory(mut self, size: AlignedUsize) -> Self {
        self.config.shared_memory = size;
        self
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.config.debug = debug;
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
