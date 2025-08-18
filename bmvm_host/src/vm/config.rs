use crate::{DEFAULT_SHARED_GUEST, DEFAULT_SHARED_HOST, GUEST_DEFAULT_STACK_SIZE};
use bmvm_common::mem::AlignedNonZeroUsize;

pub struct Config {
    pub(crate) stack_size: AlignedNonZeroUsize,
    pub(crate) shared_guest: AlignedNonZeroUsize,
    pub(crate) shared_host: AlignedNonZeroUsize,
    max_memory: u64,
    pub(crate) debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stack_size: AlignedNonZeroUsize::new_ceil(GUEST_DEFAULT_STACK_SIZE).unwrap(),
            shared_guest: AlignedNonZeroUsize::new_ceil(DEFAULT_SHARED_GUEST).unwrap(),
            shared_host: AlignedNonZeroUsize::new_ceil(DEFAULT_SHARED_HOST).unwrap(),
            max_memory: 128 * 1024 * 1024, // 128MiB
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

    pub fn max_memory(mut self, size: u64) -> Self {
        self.config.max_memory = size;
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
