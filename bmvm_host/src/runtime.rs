struct Config {
    stack_size: u64,
    max_memory: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config{
            stack_size: 2 * 1024 * 1024, // 2MiB
            max_memory: 128 * 1024 * 1024, // 128MiB
        }
    }
}

struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        ConfigBuilder{
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

struct Runtime {

}

impl Runtime {
    pub fn new() -> Self {
        Runtime{}
    }
    
    

    pub fn run(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}