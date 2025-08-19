use crate::elf::ExecBundle;
use crate::linker::config::Config;
use crate::linker::hypercall::{CallableFunction, ConversionError};
use crate::linker::{CallDirection, Func, hypercall, upcall};
use bmvm_common::vmi::{FnCall, FnPtr, Signature};
use rustc_hash::{FxBuildHasher, FxHashMap as HashMap, FxHashSet as HashSet};
use std::ffi::{CStr, CString};
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
    /// Error when the guest expects a hypercall that is not implemented by the host.
    #[error("Missing implementation for hypercall: '{func}'")]
    MissingHypercallImpl { func: FnCall },
    /// Error when the host expects an upcall that is not implemented by the guest.
    #[error("Missing implementation for upcall: '{func}'")]
    MissingUpcallImpl { func: Func },
    /// Error when a function is found in both guest and host, but their signatures do not match.
    #[error("Signature mismatch for function: Guest='{guest}' Host='{host}'.")]
    SignatureMismatch { guest: FnCall, host: Func },
    /// Error when a host function exists but is not used by the guest.
    #[error("Unused host function '{func}")]
    HostFunctionUnused { func: Func },
    /// Error when a guest function exists but is not used by the host.
    #[error("Unused guest function '{func}'")]
    GuestFunctionUnused { func: FnCall },
    /// Error when a function on the host side was found more than once.
    #[error(
        "Duplicate Host Function: direction: {direction:?}, func: {func:?}. To fix, try using a different name for the function."
    )]
    DuplicateHostFunction {
        direction: CallDirection,
        func: Func,
    },
    #[error(
        "Duplicate Guest Function: direction: {direction:?}, func: {func:?}. To fix, try using a different name for the function."
    )]
    DuplicateGuestFunction {
        direction: CallDirection,
        func: FnCall,
    },
    #[error(
        "Signature collision in guest functions: [{funcs}]. Try using a different names for the functions."
    )]
    GuestSignatureCollision { funcs: GuestFnCollision },
    #[error(
        "Signature collision in host functions: [{funcs}]. Try using a different names for the functions."
    )]
    HostSignatureCollision { funcs: HostFnCollision },
    /// Error if parsing the function metadata for a host-exposed function
    #[error("Unable to parse function metadata: {0}")]
    ParseError(#[from] ConversionError),
    /// A collection of multiple linking errors.
    #[error("Multiple linking errors occurred: {0:?}")]
    Joined(Errors<Error>),
}

#[derive(Debug)]
pub struct GuestFnCollision(Vec<FnCall>);

impl From<Vec<&FnCall>> for GuestFnCollision {
    fn from(funcs: Vec<&FnCall>) -> Self {
        let funcs = funcs
            .iter()
            .map(|f| f.to_owned().clone())
            .collect::<Vec<FnCall>>();
        Self(funcs)
    }
}

impl Display for GuestFnCollision {
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
pub struct HostFnCollision(Vec<Func>);

impl From<Vec<&Func>> for HostFnCollision {
    fn from(funcs: Vec<&Func>) -> Self {
        let funcs = funcs
            .iter()
            .map(|f| f.to_owned().clone())
            .collect::<Vec<Func>>();
        Self(funcs)
    }
}

impl Display for HostFnCollision {
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

impl Error {
    pub fn with_errors<T>(ok: T, mut errors: Vec<Error>) -> Result<T> {
        match errors.len() {
            0 => Ok(ok),
            1 => Err(errors.remove(0)),
            _ => Err(errors.into()),
        }
    }
}

impl From<Vec<Error>> for Error {
    fn from(errors: Vec<Error>) -> Self {
        if errors.len() == 1 {
            return errors.into_iter().next().unwrap();
        }

        Error::Joined(Errors::new(errors).unwrap())
    }
}

pub struct Linker {
    cfg: Config,
    hypercalls: Vec<hypercall::Function>,
}

impl Linker {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            cfg,
            hypercalls: Vec::new(),
        })
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
    pub(crate) fn link(&mut self, bundle: &ExecBundle) -> Result<()> {
        self.hypercalls = inventory::iter::<CallableFunction>()
            .map(hypercall::Function::try_from)
            .try_collect::<Vec<hypercall::Function>>()?;

        self.link_hypercall(&bundle.host)?;
        self.link_upcall(&bundle)?;

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
    fn link_upcall(&mut self, bundle: &ExecBundle) -> Result<()> {
        let result = ValidationResults::new(&self.cfg.upcalls, &bundle.expose, |f| &f.base);
        let _ = result.into_error((), CallDirection::HostToGuest, self.cfg.error_unused_guest)?;

        // TODO: include in first pass

        let mut errs = Vec::new();
        let mut hashed_upcalls: HashMap<Signature, FnPtr> =
            HashMap::with_capacity_and_hasher(bundle.upcalls.len(), FxBuildHasher::default());
        hashed_upcalls.extend(bundle.upcalls.iter().map(|f| (f.sig, f.func)));
        for upcall in &mut self.cfg.upcalls {
            match hashed_upcalls.get(&upcall.base.sig) {
                Some(ptr) => upcall.link(ptr.clone()),
                None => errs.push(Error::MissingUpcallImpl {
                    func: upcall.base.clone(),
                }),
            }
        }

        Error::with_errors((), errs)
    }

    pub(crate) fn into_calls(self) -> (Vec<upcall::Function>, Vec<hypercall::Function>) {
        (self.cfg.upcalls, self.hypercalls)
    }

    /// Link the expected hypercalls by the guest actually provided implementations by the host.
    ///
    /// This function checks for:
    /// 1. Is there a signature collision between any guest hypercalls?
    /// 2. Is there a signature collision between any host hypercalls?
    /// 3. Are all expected hypercalls implemented by the host?
    /// 4. Does the host provide any hypercalls that are not used by the guest?
    ///
    /// # Arguments
    /// * `host` - A slice of `hypercall::Function` representing the hypercalls implemented by the host.
    /// * `guest` - A slice of `FnCall` representing the hypercalls expected by the guest.
    ///
    /// # Returns
    /// - `Ok(())` if all links are valid.
    /// - `Err(Error)` if a single error occurred
    /// - `Err(Error::Joined)` if multiple errors occurred
    fn link_hypercall(&self, guest: &[FnCall]) -> Result<()> {
        let result = ValidationResults::new(&self.hypercalls, guest, |f| &f.func);
        result.into_error((), CallDirection::GuestToHost, self.cfg.error_unused_host)
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

impl<'a> ValidationResults<'a> {
    fn new<T>(host: &'a [T], guest: &'a [FnCall], extract: fn(&'a T) -> &'a Func) -> Self {
        // pre-alloc collections with estimated capacities
        let mut this = ValidationResults {
            guest_sig_collisions: HashMap::with_capacity_and_hasher(
                guest.len() / 2,
                FxBuildHasher::default(),
            ),
            host_sig_collisions: HashMap::with_capacity_and_hasher(
                host.len() / 2,
                FxBuildHasher::default(),
            ),
            unmatched_guest: Vec::with_capacity(guest.len().min(4)),
            sig_mismatches: Vec::with_capacity(guest.len().min(4)),
            unmatched_host: Vec::with_capacity(host.len().min(4)),
        };

        // lookup structures for host and guest
        let (host_by_name, host_sigs) = Self::host_maps(host, extract);
        let (guest_by_name, guest_sigs) = Self::guest_map(guest);

        // check signature collisions
        this.guest_sig_collisions = Self::find_collisions(&guest_sigs);
        this.host_sig_collisions = Self::find_collisions(&host_sigs);

        // check for mismatches
        this.analyze_matches(&host_by_name, &guest_by_name);

        this
    }

    fn is_empty(&self) -> bool {
        self.guest_sig_collisions.is_empty()
            && self.host_sig_collisions.is_empty()
            && self.unmatched_guest.is_empty()
            && self.unmatched_host.is_empty()
            && self.sig_mismatches.is_empty()
    }

    fn into_error<T>(self, value: T, direction: CallDirection, err: bool) -> Result<T> {
        if self.is_empty() {
            return Ok(value);
        }

        let mut errors = Vec::new();

        // map potential guest collisions to errors
        if !&self.guest_sig_collisions.is_empty() {
            let errs =
                self.guest_sig_collisions
                    .iter()
                    .map(|(_, c)| Error::GuestSignatureCollision {
                        funcs: GuestFnCollision::from(c.to_vec()),
                    });
            errors.extend(errs);
        }

        // map potential host collisions to errors
        if !&self.host_sig_collisions.is_empty() {
            let errs =
                self.host_sig_collisions
                    .iter()
                    .map(|(_, c)| Error::HostSignatureCollision {
                        funcs: HostFnCollision::from(c.to_vec()),
                    });
            errors.extend(errs);
        }

        // map signature mismatches to errors
        if !&self.sig_mismatches.is_empty() {
            let errs = self
                .sig_mismatches
                .iter()
                .map(|(g, h)| Error::SignatureMismatch {
                    guest: g.to_owned().clone(),
                    host: h.to_owned().clone(),
                });
            errors.extend(errs);
        }

        match direction {
            CallDirection::HostToGuest => {
                // map unused host functions to errors
                if !&self.unmatched_host.is_empty() {
                    for f in self.unmatched_host.iter() {
                        let err = Error::MissingUpcallImpl {
                            func: f.to_owned().clone(),
                        };
                        errors.push(err);
                    }
                }

                // map unused guest function to either log::warn or errors depending on configuration
                if !self.unmatched_guest.is_empty() {
                    if err {
                        let errs =
                            self.unmatched_guest
                                .iter()
                                .map(|f| Error::GuestFunctionUnused {
                                    func: f.to_owned().clone(),
                                });
                        errors.extend(errs);
                    } else {
                        self.unmatched_guest.iter().for_each(|f| {
                            log::warn!("Guest function '{}' is not used by host.", f)
                        });
                    }
                }
            }
            CallDirection::GuestToHost => {
                // map unused guest functions to errors
                if !&self.unmatched_guest.is_empty() {
                    for f in self.unmatched_guest.iter() {
                        let err = Error::MissingHypercallImpl {
                            func: f.to_owned().clone(),
                        };
                        errors.push(err);
                    }
                }

                // map unused host function to either log::warn or errors depending on configuration
                if !self.unmatched_host.is_empty() {
                    if err {
                        let errs = self
                            .unmatched_host
                            .iter()
                            .map(|f| Error::HostFunctionUnused {
                                func: f.to_owned().clone(),
                            });
                        errors.extend(errs);
                    } else {
                        self.unmatched_host.iter().for_each(|f| {
                            log::warn!("Host function '{}' is not used by guest.", f)
                        });
                    }
                }
            }
        }

        Error::with_errors(value, errors)
    }

    fn host_maps<T>(
        host: &'a [T],
        extract: fn(&'a T) -> &'a Func,
    ) -> (
        HashMap<&'a str, &'a Func>,
        HashMap<Signature, Vec<&'a Func>>,
    ) {
        let mut by_name =
            HashMap::<&str, &Func>::with_capacity_and_hasher(host.len(), FxBuildHasher::default());
        let mut by_sig = HashMap::<Signature, Vec<&Func>>::with_capacity_and_hasher(
            host.len(),
            FxBuildHasher::default(),
        );

        for f in host {
            let func = extract(f);
            by_name.insert(func.name.as_str(), func);
            by_sig.entry(func.sig).or_default().push(func);
        }

        (by_name, by_sig)
    }

    fn guest_map(guest: &[FnCall]) -> (HashMap<&CStr, &FnCall>, HashMap<Signature, Vec<&FnCall>>) {
        let mut by_name = HashMap::<&CStr, &FnCall>::with_capacity_and_hasher(
            guest.len(),
            FxBuildHasher::default(),
        );
        let mut by_sig = HashMap::<Signature, Vec<&FnCall>>::with_capacity_and_hasher(
            guest.len(),
            FxBuildHasher::default(),
        );

        for call in guest {
            by_name.insert(call.name.as_c_str(), call);
            by_sig.entry(call.sig).or_default().push(call);
        }

        (by_name, by_sig)
    }

    fn find_collisions<T>(
        items: &HashMap<Signature, Vec<&'a T>>,
    ) -> HashMap<Signature, Vec<&'a T>> {
        items
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(&k, v)| (k, v.clone()))
            .collect()
    }

    fn analyze_matches(
        &mut self,
        host_by_name: &HashMap<&'a str, &'a Func>,
        guest_by_name: &HashMap<&'a CStr, &'a FnCall>,
    ) {
        // Track matched functions to find unmatched ones later
        let mut matched_functions =
            HashSet::with_capacity_and_hasher(host_by_name.len(), FxBuildHasher::default());

        // Check guest calls against host functions
        for (n, &call) in guest_by_name {
            let name = n.to_str().unwrap();
            match host_by_name.get(name) {
                Some(&func) => {
                    if call.sig != func.sig {
                        self.sig_mismatches.push((call, func));
                    }
                    matched_functions.insert(&func.name);
                }
                None => self.unmatched_guest.push(call),
            }
        }

        // Find functions without matching calls
        for (name, &func) in host_by_name {
            if !guest_by_name.contains_key(CString::new(*name).unwrap().as_c_str()) {
                self.unmatched_host.push(func);
            }
        }
    }
}
