use crate::elf::ExecBundle;
use bmvm_common::vmi::{FnCall, HostVmiFn, Signature, Usage};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

type Result<T> = std::result::Result<T, Error>;

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
    /// Error when a host function exists but is not utilized by the guest.
    #[error("Host function '{name}' is not used by guest.")]
    HostFunctionUnused { name: String },
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
    /// and host-exposed functions against guest implementations.
    ///
    /// Two types of validations are executed:
    /// 1. It checks if all functions the guest intends to `call` on the host side are present and
    /// their respective function signatures match.
    /// 2. It checks if all `implementations` provided by the guest match their respective function
    /// signatures.
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
        let mut calls = Vec::new();
        let mut implementations = Vec::new();
        for func in inventory::iter::<HostVmiFn> {
            match func.usage {
                Usage::Impl => implementations.push(func.call.clone()),
                Usage::Call => calls.push(func.call.clone()),
            }
        }

        calls.sort();
        implementations.sort();

        let mut errors: Vec<Error> = Vec::new();
        match self.validate_links(&calls, &bundle.host) {
            Ok(_) => {}
            Err(e) => errors.push(e),
        };

        match self.validate_links(&implementations, &bundle.expose) {
            Ok(_) => {}
            Err(e) => errors.push(e),
        }

        // If no errors were collected, return Ok(()). Otherwise, return the collected errors.
        match errors.len() {
            0 => Ok(()),
            _ => Err(errors.into()),
        }
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
    /// * `host` - A slice of `FnCall` representing the functions available in the host.
    /// * `guest` - A slice of `FnCall` representing the functions expected/used by the guest.
    ///
    /// # Returns
    /// - `Ok(())` if all links are valid.
    /// - `Err(Error)` if a single error occurred
    /// - `Err(Error::Joined)` if multiple errors occurred
    fn validate_links(&self, host: &[FnCall], guest: &[FnCall]) -> Result<()> {
        let mut errors: Vec<Error> = Vec::new();

        // Create a HashMap for efficient lookup of host functions by name and a HashSet to track which host functions are used by the guest.
        let host_map: HashMap<String, Signature> = host
            .iter()
            // Convert CString name to String for HashMap key. Using unwrap() here assumes valid UTF-8 names.
            .map(|f| (f.name.to_str().unwrap().to_string(), f.sig))
            .collect();

        let mut used_host_functions: HashSet<String> = HashSet::new();

        // Iterate through guest functions to find missing ones or signature mismatches.
        for guest_fn in guest {
            let guest_name = guest_fn.name.to_str().unwrap().to_string();
            match host_map.get(&guest_name) {
                Some(&host_sig) => {
                    // Function found in host, mark its name as used.
                    used_host_functions.insert(guest_name.clone());
                    // Check for signature mismatch.
                    if guest_fn.sig != host_sig {
                        errors.push(Error::SignatureMismatch {
                            name: guest_name,
                            guest: guest_fn.sig,
                            host: host_sig,
                        });
                    }
                }
                None => {
                    // Guest function not found in host.
                    errors.push(Error::GuestFunctionMissing {
                        name: guest_name,
                        sig: guest_fn.sig,
                    });
                }
            }
        }

        // Iterate through host functions to find any that are not utilized by the guest.
        for host_fn in host {
            let host_name = host_fn.name.to_str().unwrap().to_string();
            if !used_host_functions.contains(&host_name) {
                if !self.cfg.error_unused_host_functions {
                    log::warn!("Host function '{host_name}' is not used by guest.");
                } else {
                    errors.push(Error::HostFunctionUnused { name: host_name });
                }
            }
        }

        // If no errors were collected, return Ok(()). Otherwise, return the collected errors.
        match errors.len() {
            0 => Ok(()),
            _ => Err(errors.into()),
        }
    }
}
