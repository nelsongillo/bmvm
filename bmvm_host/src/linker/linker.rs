use crate::elf::ExecBundle;
use crate::linker::{CallableFunction, ConversionError, Function, Params};
use bmvm_common::vmi::{FnCall, Signature};
use bmvm_common::{TypeSignature, vmi};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Errors<T: core::error::Error>(Vec<T>);

impl<T: core::error::Error> Errors<T> {
    pub fn new(errors: Vec<T>) -> Option<Self> {
        if errors.is_empty() {
            return None;
        }

        Some(Self(errors))
    }

    pub fn new_unchecked(errors: Vec<T>) -> Self {
        Self(errors)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.0.iter()
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

impl<T: core::error::Error> Display for Errors<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Multiple errors occurred: ")?;
        for err in &self.0 {
            write!(f, "{};", err)?;
        }
        Ok(())
    }
}

/// Defines possible errors that can occur during function linking.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error when a guest function is expected but not found in the host.
    #[error("Guest function '{name}' (signature: {sig:#x}) is missing in host.")]
    GuestFunctionMissing { name: String, sig: Signature },
    /// Error when a function is found in both guest and host, but their signatures do not match.
    #[error("Signature mismatch for function '{name}': guest has {guest:#x}, host has {host:#x}.")]
    SignatureMismatch {
        name: String,
        guest: Signature,
        host: Signature,
    },
    /// Error when a host function exists but is not used by the guest.
    #[error("Host function '{name}' is not used by guest.")]
    HostFunctionUnused { name: String },
    /// Error if parsing the function metadata for a host-exposed function
    #[error("Unable to parse function metadata: {0}")]
    ParseError(#[from] ConversionError),
    /// A collection of multiple linking errors.
    #[error("Multiple linking errors occurred: {0:?}")]
    Joined(Errors<Error>),
}

impl From<Vec<Error>> for Error {
    fn from(errors: Vec<Error>) -> Self {
        if errors.len() == 1 {
            return errors.into_iter().next().unwrap();
        }

        Error::Joined(Errors::new(errors).unwrap())
    }
}

#[derive(Debug)]
pub struct Config {
    error_unused_host_functions: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            error_unused_host_functions: true,
        }
    }
}

pub struct Linker {
    cfg: Config,
}

impl Linker {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }

    /// Performs the linking process by validating guest function calls against host implementations
    /// It checks if all functions the guest intends to `call` on the host side are present and
    /// their respective function signatures match.
    ///
    /// # Arguments
    /// * `bundle` - A reference to an `ExecBundle` containing the parsed guest execution context.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if all linking validations pass successfully.
    /// * `Err(Error)` containing a detailed list of all linking
    ///   errors encountered if any validation fails.
    pub(crate) fn link(&self, bundle: &ExecBundle) -> Result<()> {
        let mut calls: Vec<Function> = inventory::iter::<CallableFunction>()
            .map(Function::try_from)
            .try_collect::<Vec<Function>>()?;
        calls.sort();

        self.validate_links(&calls, &bundle.host)
    }

    /// Validates the links between guest and host functions.
    ///
    /// This function checks for:
    /// 1. Guest functions that are missing in the host.
    /// 2. Guest functions that have a signature mismatch with their host counterparts.
    /// 3. Host functions that are not used by the guest. (Warn/Error depending on `Config`)
    ///
    /// All identified issues are collected and returned as a single `CollectedErrors`
    /// if any problems are found.
    ///
    /// # Arguments
    /// * `host` - A slice of `Function` representing the functions available in the host.
    /// * `guest` - A slice of `FnCall` representing the functions expected/used by the guest.
    ///
    /// # Returns
    /// - `Ok(())` if all links are valid.
    /// - `Err(Error)` if a single error occurred
    /// - `Err(Error::Joined)` if multiple errors occurred
    fn validate_links(&self, host: &[Function], guest: &[FnCall]) -> Result<()> {
        let mut errors: Vec<Error> = Vec::new();

        let host_map: HashMap<String, Signature> =
            host.iter().map(|f| (f.name.clone(), f.sig)).collect();

        let mut used_host_functions: HashSet<String> = HashSet::new();

        // iter through guest functions to find signature mismatches/missing ones
        for guest_fn in guest {
            let guest_name = guest_fn.name.to_str().unwrap().to_string();
            match host_map.get(&guest_name) {
                Some(&host_sig) => {
                    // Function found in host, mark its name as used
                    used_host_functions.insert(guest_name.clone());
                    // Check for signature mismatch
                    if guest_fn.sig != host_sig {
                        errors.push(Error::SignatureMismatch {
                            name: guest_name,
                            guest: guest_fn.sig,
                            host: host_sig,
                        });
                    }
                }
                None => {
                    // Guest function not found in host
                    errors.push(Error::GuestFunctionMissing {
                        name: guest_name,
                        sig: guest_fn.sig,
                    });
                }
            }
        }

        // Iterate through host functions to find any that are not used by the guest.
        for host_fn in host {
            if !used_host_functions.contains(&host_fn.name) {
                if !self.cfg.error_unused_host_functions {
                    log::warn!("Host function '{}' is not used by guest.", host_fn.name);
                } else {
                    errors.push(Error::HostFunctionUnused {
                        name: host_fn.name.clone(),
                    });
                }
            }
        }

        // Join present errors
        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.remove(0)),
            _ => Err(errors.into()),
        }
    }
}
