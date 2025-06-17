#[derive(Debug)]
pub struct Func {}

pub struct Linker {}

impl Linker {
    pub fn new(_cfg: LinkerConfig) -> Self {
        Self {}
    }
}

#[derive(Debug)]
pub struct LinkerConfig {
    discover: bool,
    include: Option<Vec<Func>>,
    exclude: Option<Vec<Func>>,
}

impl Default for LinkerConfig {
    fn default() -> Self {
        Self {
            discover: true,
            include: None,
            exclude: None,
        }
    }
}

pub struct LinkerConfigBuilder {
    discover: bool,
    include: Vec<Func>,
    exclude: Vec<Func>,
}

impl LinkerConfigBuilder {
    /// Constructor with `discover = true`
    pub fn new() -> Self {
        Self {
            discover: true,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }

    pub fn discover(mut self, discover: bool) -> Self {
        self.discover = discover;
        self
    }

    /// Include a single item
    pub fn include<S>(mut self, value: S) -> Self
    where
        S: Into<Func>,
    {
        self.include.push(value.into());
        self
    }

    /// Include multiple items
    pub fn include_many<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Func>,
    {
        self.include.extend(values.into_iter().map(Into::into));
        self
    }

    /// Exclude a single item
    pub fn exclude<S>(mut self, value: S) -> Self
    where
        S: Into<Func>,
    {
        self.exclude.push(value.into());
        self
    }

    /// Exclude multiple items (iterator)
    pub fn exclude_many<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Func>,
    {
        self.exclude.extend(values.into_iter().map(Into::into));
        self
    }

    /// Finalize and build the `LinkerOptions` object
    pub fn build(self) -> LinkerConfig {
        LinkerConfig {
            discover: self.discover,
            include: if self.include.is_empty() {
                None
            } else {
                Some(self.include)
            },
            exclude: if self.exclude.is_empty() {
                None
            } else {
                Some(self.exclude)
            },
        }
    }
}
