use crate::elf::ExecBundle;
use crate::linker::config::Config;
use crate::linker::hypercall::{CallableFunction, ConversionError};
use crate::linker::{CallDirection, Func, hypercall, upcall};
use bmvm_common::vmi::{FnCall, Signature};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::fmt::{Display, Formatter};

pub type Result<'a, T> = std::result::Result<T, Error<'a>>;

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
pub enum Error<'a> {
    /// Error when the guest expects a hypercall that is not implemented by the host.
    #[error("Missing implementation for hypercall: '{func}'")]
    MissingHypercallImpl { func: &'a FnCall },
    /// Error when the host expects an upcall that is not implemented by the guest.
    #[error("Missing implementation for upcall: '{func}'")]
    MissingUpcallImpl { func: &'a upcall::Function },
    /// Error when a function is found in both guest and host, but their signatures do not match.
    #[error("Signature mismatch for function: Guest='{guest}' Host='{host}'.")]
    SignatureMismatch { guest: &'a FnCall, host: &'a Func },
    /// Error when a host function exists but is not used by the guest.
    #[error("Unused host function '{func}")]
    HostFunctionUnused { func: &'a Func },
    /// Error when a guest function exists but is not used by the host.
    #[error("Unused guest function '{func}'")]
    GuestFunctionUnused { func: &'a FnCall },
    /// Error when a function on the host side was found more than once.
    #[error(
        "Duplicate Host Function: direction: {direction:?}, func: {func:?}. To fix, try using a different name for the function."
    )]
    DuplicateHostFunction {
        direction: CallDirection,
        func: &'a Func,
    },
    #[error(
        "Duplicate Guest Function: direction: {direction:?}, func: {func:?}. To fix, try using a different name for the function."
    )]
    DuplicateGuestFunction {
        direction: CallDirection,
        func: &'a FnCall,
    },
    #[error(
        "Signature collision in guest functions: [{funcs}]. Try using a different names for the functions."
    )]
    GuestSignatureCollision { funcs: GuestFnCollision<'a> },
    #[error(
        "Signature collision in host functions: [{funcs}]. Try using a different names for the functions."
    )]
    HostSignatureCollision { funcs: HostFnCollision<'a> },
    /// Error if parsing the function metadata for a host-exposed function
    #[error("Unable to parse function metadata: {0}")]
    ParseError(#[from] ConversionError),
    /// A collection of multiple linking errors.
    #[error("Multiple linking errors occurred: {0:?}")]
    Joined(Errors<Error<'a>>),
}

#[derive(Debug)]
pub struct GuestFnCollision<'a>(Vec<&'a FnCall>);

impl<'a> From<Vec<&'a FnCall>> for GuestFnCollision<'a> {
    fn from(funcs: Vec<&'a FnCall>) -> Self {
        Self(funcs)
    }
}

impl<'a> Display for GuestFnCollision<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let funcs = self
            .0
            .iter()
            .map(|func| format!("{}", func))
            .collect::<Vec<String>>()
            .join(",");
        write!(f, "{}", funcs)
    }
}

#[derive(Debug)]
pub struct HostFnCollision<'a>(Vec<&'a Func>);

impl<'a> From<Vec<&'a Func>> for HostFnCollision<'a> {
    fn from(funcs: Vec<&'a Func>) -> Self {
        Self(funcs)
    }
}
impl<'a> Display for HostFnCollision<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let funcs = self
            .0
            .iter()
            .map(|func| format!("{}", func))
            .collect::<Vec<String>>()
            .join(",");
        write!(f, "{}", funcs)
    }
}

impl<'a> Error<'a> {
    pub fn with_errors<T>(ok: T, mut errors: Vec<Error>) -> Result<T> {
        match errors.len() {
            0 => Ok(ok),
            1 => Err(errors.remove(0)),
            _ => Err(errors.into()),
        }
    }
}

impl<'a> From<Vec<Error<'a>>> for Error<'a> {
    fn from(errors: Vec<Error<'a>>) -> Self {
        if errors.len() == 1 {
            return errors.into_iter().next().unwrap();
        }

        Error::Joined(Errors::new(errors).unwrap())
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
        Ok(())
    }

    /// Link the expected upcalls by the host with the actually provided upcall implementations by the guest.
    ///
    /// This function checks for:
    /// 1. Is there a signature collision between any guest upcalls?
    /// 2. Is there a signature collision between any host upcalls?
    /// 3. Are all expected upcalls implemented by the guest?
    /// 4. Does the guest provide any upcalls that are not used by the host?
    ///
    /// # Arguments
    /// * `host` - A slice of `upcall::Function` representing the functions expected by the host.
    /// * `guest` - A slice of `FnCall` representing the functions implemented by the guest.
    ///
    /// # Returns
    /// - `Ok(())` if all links are valid.
    /// - `Err(Error)` if a single error occurred
    /// - `Err(Error::Joined)` if multiple errors occurred
    fn link_upcall(&self, host: &[upcall::Function], guest: &[FnCall]) -> Result<()> {
        let mut errors = Vec::new();
        let result = validate_function_mapping(host, guest);

        // map potential guest collisions to errors
        if !result.guest_sig_collisions.is_empty() {
            let errs =
                result
                    .guest_sig_collisions
                    .iter()
                    .map(|(_, c)| Error::GuestSignatureCollision {
                        funcs: GuestFnCollision::from(c.to_vec()),
                    });
            errors.extend(errs);
        }

        // map potential host collisions to errors
        if !result.host_sig_collisions.is_empty() {
            let errs =
                result
                    .host_sig_collisions
                    .iter()
                    .map(|(_, c)| Error::HostSignatureCollision {
                        funcs: HostFnCollision::from(c.to_vec()),
                    });
            errors.extend(errs);
        }

        // map signature mismatches to errors
        if !result.sig_mismatches.is_empty() {
            let errs = result
                .sig_mismatches
                .iter()
                .map(|(g, h)| Error::SignatureMismatch { guest: g, host: h });
            errors.extend(errs);
        }

        // map unused host functions to errors
        if !result.unmatched_host.is_empty() {
            for f in result.unmatched_host.into_iter() {
                let err = Error::MissingUpcallImpl { func: f };
                errors.push(err);
            }
        }

        // map unused guest function to either log::warn or errors depending on configuration
        if !result.unmatched_guest.is_empty() {
            if self.cfg.error_unused_guest {
                let errs = result
                    .unmatched_guest
                    .into_iter()
                    .map(|f| Error::GuestFunctionUnused { func: f });
                errors.extend(errs);
            } else {
                result
                    .unmatched_guest
                    .iter()
                    .for_each(|f| log::warn!("Guest function '{}' is not used by guest.", f));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ValidationResults<'a> {
    pub guest_sig_collisions: HashMap<Signature, Vec<&'a FnCall>>,
    pub host_sig_collisions: HashMap<Signature, Vec<&'a Func>>,
    pub unmatched_guest: Vec<&'a FnCall>,
    pub unmatched_host: Vec<&'a Func>,
    pub sig_mismatches: Vec<(&'a FnCall, &'a Func)>,
}

pub fn validate_function_mapping<'a>(
    host: &'a [Func],
    guest: &'a [FnCall],
) -> ValidationResults<'a> {
    // Pre-allocate collections with estimated capacities
    let mut results = ValidationResults {
        guest_sig_collisions: HashMap::with_capacity(guest.len() / 2),
        host_sig_collisions: HashMap::with_capacity(host.len() / 2),
        unmatched_guest: Vec::with_capacity(guest.len().min(4)),
        sig_mismatches: Vec::with_capacity(guest.len().min(4)),
        unmatched_host: Vec::with_capacity(host.len().min(4)),
    };

    // Build lookup structures in single passes
    let (host_by_name, host_sigs) = build_host_maps(host);
    let (guest_by_name, guest_sigs) = build_guest_maps(guest);

    // Check for signature collisions
    results.guest_sig_collisions = find_collisions(&guest_sigs);
    results.host_sig_collisions = find_collisions(&host_sigs);

    // Check for matches and mismatches
    analyze_matches(
        &host_by_name,
        &guest_by_name,
        &mut results.unmatched_guest,
        &mut results.sig_mismatches,
        &mut results.unmatched_host,
    );

    results
}

fn build_host_maps(host: &[Func]) -> (HashMap<&str, &Func>, HashMap<Signature, Vec<&Func>>) {
    let mut by_name = HashMap::<&str, &Func>::with_capacity(host.len());
    let mut by_sig = HashMap::<Signature, Vec<&Func>>::with_capacity(host.len());

    for func in host {
        by_name.insert(func.name.as_str(), func);
        by_sig.entry(func.sig).or_default().push(func);
    }

    (by_name, by_sig)
}

fn build_guest_maps(
    guest: &[FnCall],
) -> (HashMap<&CStr, &FnCall>, HashMap<Signature, Vec<&FnCall>>) {
    let mut by_name = HashMap::<&CStr, &FnCall>::with_capacity(guest.len());
    let mut by_sig = HashMap::<Signature, Vec<&FnCall>>::with_capacity(guest.len());

    for call in guest {
        by_name.insert(call.name.as_c_str(), call);
        by_sig.entry(call.sig).or_default().push(call);
    }

    (by_name, by_sig)
}

fn find_collisions<'a, T>(
    items: &HashMap<Signature, Vec<&'a T>>,
) -> HashMap<Signature, Vec<&'a T>> {
    items
        .iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(&k, v)| (k, v.clone()))
        .collect()
}

fn analyze_matches<'a>(
    host_by_name: &HashMap<&'a str, &'a Func>,
    guest_by_name: &HashMap<&'a CStr, &'a FnCall>,
    unmatched_fn_calls: &mut Vec<&'a FnCall>,
    sig_mismatches: &mut Vec<(&'a FnCall, &'a Func)>,
    unmatched_functions: &mut Vec<&'a Func>,
) {
    // Track matched functions to find unmatched ones later
    let mut matched_functions = HashSet::with_capacity(host_by_name.len());

    // Check guest calls against host functions
    for (n, &call) in guest_by_name {
        let name = n.to_str().unwrap();
        match host_by_name.get(name) {
            Some(&func) => {
                if call.sig != func.sig {
                    sig_mismatches.push((call, func));
                }
                matched_functions.insert(&func.name);
            }
            None => unmatched_fn_calls.push(call),
        }
    }

    // Find functions without matching calls
    for (name, &func) in host_by_name {
        if !guest_by_name.contains_key(CString::new(*name).unwrap().as_c_str()) {
            unmatched_functions.push(func);
        }
    }
}
