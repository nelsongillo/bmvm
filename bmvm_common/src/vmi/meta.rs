use crate::vmi::Signature;
use core::cmp::Ordering;
use std::ffi::{CStr, CString, FromVecWithNulError, NulError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("provided buffer is too short: expected at least {expected} bytes, got {actual}")]
    TooShort { expected: usize, actual: usize },
    #[error("parsed signature is zero")]
    ZeroSignature,
    #[error("empty function name")]
    EmptyFunctionName,
    #[error("empty parameter type")]
    EmptyParameterType,
    #[error("missing null termination in string")]
    MissingNullTermination,
    #[error("failed to parse string")]
    StringParsingError(#[from] FromVecWithNulError),
    #[error("string contains invalid characters")]
    InvalidString(#[from] NulError),
    #[error("too many parameters: supported are up to {max} parameters, got {actual}")]
    TooManyParameters { max: usize, actual: usize },
    #[error("too few parameters: expected {expected}, got {actual}")]
    TooFewParameters { expected: usize, actual: usize },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FnCall {
    pub sig: Signature,
    pub name: CString,
    #[cfg(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    ))]
    pub debug_param_types: Vec<CString>,
    #[cfg(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    ))]
    pub debug_return_type: Option<CString>,
}

impl FnCall {
    /// Serialize the `FnCall` to a byte vector, including debug information if either build in
    /// debug mode, or one of the following features are enabled: `vmi-debug`, `vmi-consume`.
    /// The `vmi-no-debug` feature overwrites the other features and enforces the omission of the
    /// debug information.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend(&self.sig.to_ne_bytes());
        buf.extend(self.name.as_bytes_with_nul());

        // serialize debug info (only in debug builds or if explicitly requested)
        #[cfg(any(
            all(debug_assertions, not(feature = "vmi-no-debug")),
            all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
            feature = "vmi-consume",
        ))]
        {
            // param count
            buf.push(self.debug_param_types.len() as u8);
            // serialize each param as CString
            for param in &self.debug_param_types {
                buf.extend(param.as_bytes_with_nul());
            }

            // return type
            match &self.debug_return_type {
                Some(rt) => buf.extend(rt.as_bytes_with_nul()),
                None => buf.push(0),
            }
        }

        buf
    }

    pub fn signature(&self) -> u64 {
        self.sig
    }

    pub fn name(&self) -> &CStr {
        self.name.as_c_str()
    }
}

#[cfg(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
))]
impl FnCall {
    pub fn new<S>(
        sig: Signature,
        fn_name: S,
        params: &[S],
        return_type: Option<S>,
    ) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        if sig == 0 {
            return Err(Error::ZeroSignature);
        }

        if fn_name.as_ref().is_empty() {
            return Err(Error::EmptyFunctionName);
        }
        let name = CString::new(fn_name.as_ref()).map_err(|e| Error::InvalidString(e))?;

        if params.len() > u8::MAX as usize {
            return Err(Error::TooManyParameters {
                max: u8::MAX as usize,
                actual: params.len(),
            });
        }

        let mut debug_param_types = Vec::new();
        for param in params {
            if param.as_ref().is_empty() {
                return Err(Error::EmptyParameterType);
            }

            let cparam = CString::new(param.as_ref()).map_err(|e| Error::InvalidString(e))?;
            debug_param_types.push(cparam);
        }

        let debug_return_type = if let Some(rt) = return_type {
            Some(CString::new(rt.as_ref()).map_err(|e| Error::InvalidString(e))?)
        } else {
            None
        };

        Ok(FnCall {
            sig,
            name,
            debug_param_types,
            debug_return_type,
        })
    }

    pub fn params(&self) -> &[CString] {
        self.debug_param_types.as_slice()
    }

    pub fn return_type(&self) -> Option<&CString> {
        self.debug_return_type.as_ref()
    }
}

#[cfg(not(any(
    all(debug_assertions, not(feature = "vmi-no-debug")),
    all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
    feature = "vmi-consume",
)))]
impl FnCall {
    pub fn new<S>(sig: Signature, fn_name: S) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        if sig == 0 {
            return Err(Error::ZeroSignature);
        }

        if fn_name.as_ref().is_empty() {
            return Err(Error::EmptyFunctionName);
        }

        Ok(FnCall {
            sig,
            name: CString::new(fn_name.as_ref()).map_err(|e| Error::InvalidString(e))?,
        })
    }
}

#[cfg(feature = "vmi-consume")]
/// Parsing implementation
impl FnCall {
    const MIN_SIZE: usize = {
        // signature u64
        size_of::<u64>()
            // Fn Name: min len 1 + null terminator
            + size_of::<u8>() + size_of::<u8>()
    };

    const MIN_SIZE_DEBUG: usize = {
        // no parameter -> size = 0 and Union return parameter -> only null terminator
        Self::MIN_SIZE + size_of::<u8>() + size_of::<u8>()
    };

    fn try_from_bytes_consumed(buf: &[u8], debug: bool) -> Result<(Self, usize), Error> {
        if debug && buf.len() < Self::MIN_SIZE_DEBUG {
            return Err(Error::TooShort {
                expected: Self::MIN_SIZE_DEBUG,
                actual: buf.len(),
            });
        }

        if buf.len() < Self::MIN_SIZE {
            return Err(Error::TooShort {
                expected: Self::MIN_SIZE,
                actual: buf.len(),
            });
        }

        let sig = u64::from_ne_bytes(buf[0..8].try_into().unwrap());
        if sig == 0 {
            return Err(Error::ZeroSignature);
        }
        let mut offset = 8;

        // Read name CString
        let (name, o) = Self::read_cstring(&buf[offset..])?;
        offset += o;

        if debug {
            let param_count = buf[offset] as usize;
            offset += 1;

            let mut debug_param_types = Vec::with_capacity(param_count);
            for _ in 0..param_count {
                if buf.len() <= offset {
                    return Err(Error::TooFewParameters {
                        expected: param_count,
                        actual: debug_param_types.len(),
                    });
                }
                let (param, o) = Self::read_cstring(&buf[offset..])?;
                debug_param_types.push(param);
                offset += o;
            }

            // read the return type
            let (ret, o) = Self::read_cstring(&buf[offset..])?;
            offset += o;
            let debug_return_type = if ret.is_empty() { None } else { Some(ret) };
            return Ok((
                FnCall {
                    sig,
                    name,
                    debug_param_types,
                    debug_return_type,
                },
                offset,
            ));
        }

        Ok((
            FnCall {
                sig,
                name,
                debug_param_types: Vec::new(),
                debug_return_type: None,
            },
            offset,
        ))
    }

    /// Try parsing the `FnCall` from a byte buffer. If `debug` is set, the parser expects the
    /// encoded `FnCall` to contain the optional function parameter and return type.
    /// Otherwise, it will simply end after the required fields.
    pub fn try_from_bytes(buf: &[u8], debug: bool) -> Result<Self, Error> {
        Self::try_from_bytes_consumed(buf, debug).map(|(meta, _)| meta)
    }

    /// Try parsing a vector of `FnCall` from a byte buffer. If `debug` is set, the parser
    /// expects the encoded `FnCall` to contain the optional function parameter and return type.
    /// Otherwise, it will simply end after the required fields.
    pub fn try_from_bytes_vec(buf: &[u8], debug: bool) -> Result<Vec<Self>, Error> {
        let mut offset = 0;
        let mut output = Vec::new();

        while offset < buf.len() {
            let (meta, o) = Self::try_from_bytes_consumed(&buf[offset..], debug)?;
            offset += o;
            output.push(meta);
        }

        Ok(output)
    }

    fn read_cstring(input: &[u8]) -> Result<(CString, usize), Error> {
        let pos = memchr::memchr(0, input).ok_or_else(|| Error::MissingNullTermination)?;
        let str_buf = input[..pos + 1].to_vec();
        let str = CString::from_vec_with_nul(str_buf).map_err(|e| Error::StringParsingError(e))?;
        Ok((str, pos + 1))
    }
}

impl PartialOrd for FnCall {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FnCall {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

mod test {
    #![allow(unused)]

    use super::*;

    #[cfg(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    ))]
    #[test]
    fn to_bytes_debug() {
        let meta = FnCall {
            sig: 0x1234567890abcdef,
            name: CString::new("foo").unwrap(),
            debug_param_types: vec![CString::new("bar").unwrap(), CString::new("baz").unwrap()],
            debug_return_type: Some(CString::new("qux").unwrap()),
        };

        let mut expect = Vec::new();
        expect.extend(0x1234567890abcdefu64.to_le_bytes());
        expect.extend(b"foo\0");
        expect.push(2);
        expect.extend(b"bar\0");
        expect.extend(b"baz\0");
        expect.extend(b"qux\0");

        assert_eq!(expect.as_slice(), meta.to_bytes().as_slice());
    }

    #[cfg(not(any(
        all(debug_assertions, not(feature = "vmi-no-debug")),
        all(feature = "vmi-debug", not(feature = "vmi-no-debug")),
        feature = "vmi-consume",
    )))]
    #[test]
    fn to_bytes_no_debug() {
        let meta = FnCall {
            sig: 0x1234567890abcdef,
            name: CString::new("foo").unwrap(),
        };

        let mut expect = Vec::new();
        expect.extend(0x1234567890abcdefu64.to_le_bytes());
        expect.extend(b"foo\0");

        assert_eq!(expect.as_slice(), meta.to_bytes().as_slice());
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_debug_no_params_no_ret() {
        let mut buf = Vec::new();
        buf.extend(0x1234567890abcdefu64.to_le_bytes());
        buf.extend(b"foo\0");
        buf.push(0);
        buf.push(0);

        let expect = FnCall {
            sig: 0x1234567890abcdef,
            name: CString::new("foo").unwrap(),
            debug_param_types: Vec::new(),
            debug_return_type: None,
        };

        assert_eq!(
            expect,
            FnCall::try_from_bytes(buf.as_slice(), true).unwrap()
        );
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_no_debug() {
        let mut buf = Vec::new();
        buf.extend(0x1234567890abcdefu64.to_le_bytes());
        buf.extend(b"foo\0");

        let expect = FnCall {
            sig: 0x1234567890abcdef,
            name: CString::new("foo").unwrap(),
            debug_param_types: Vec::new(),
            debug_return_type: None,
        };

        assert_eq!(
            expect,
            FnCall::try_from_bytes(buf.as_slice(), false).unwrap()
        );
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_debug_2_params_and_return() {
        let mut buf = Vec::new();
        buf.extend(0x1234567890abcdefu64.to_le_bytes());
        buf.extend(b"foo\0");
        buf.push(2);
        buf.extend(b"bar\0");
        buf.extend(b"baz\0");
        buf.extend(b"qux\0");

        let expect = FnCall {
            sig: 0x1234567890abcdef,
            name: CString::new("foo").unwrap(),
            debug_param_types: vec![CString::new("bar").unwrap(), CString::new("baz").unwrap()],
            debug_return_type: Some(CString::new("qux").unwrap()),
        };

        assert_eq!(
            expect,
            FnCall::try_from_bytes(buf.as_slice(), true).unwrap()
        );
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_zero_signature() {
        let mut buf = Vec::new();
        buf.extend(0u64.to_le_bytes());
        buf.extend(b"foo\0");
        buf.push(0);
        buf.push(0);

        let expect = Error::ZeroSignature;
        let result = FnCall::try_from_bytes(buf.as_slice(), true);
        assert!(matches!(result, Err(Error::ZeroSignature)));
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_too_short_no_debug() {
        let mut buf = Vec::new();

        let result = FnCall::try_from_bytes(buf.as_slice(), false);
        assert!(matches!(
            result,
            Err(Error::TooShort {
                expected: FnCall::MIN_SIZE,
                actual: 0
            })
        ));
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_too_short_debug() {
        let mut buf = Vec::new();

        let result = FnCall::try_from_bytes(buf.as_slice(), true);
        assert!(matches!(
            result,
            Err(Error::TooShort {
                expected: FnCall::MIN_SIZE_DEBUG,
                actual: 0
            })
        ));
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_too_few_params_debug() {
        let mut buf = Vec::new();
        buf.extend(0x1234567890abcdefu64.to_le_bytes());
        buf.extend(b"foo\0");
        buf.push(2);
        buf.extend(b"bar\0");

        let result = FnCall::try_from_bytes(buf.as_slice(), true);
        assert!(matches!(
            result,
            Err(Error::TooFewParameters {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_vec_debug() {
        let expect = vec![
            FnCall {
                sig: 0x1234567890abcdef,
                name: CString::new("foo").unwrap(),
                debug_param_types: vec![CString::new("bar").unwrap()],
                debug_return_type: None,
            },
            FnCall {
                sig: 0xabcdef1234567890,
                name: CString::new("another").unwrap(),
                debug_param_types: vec![],
                debug_return_type: Some(CString::new("qux").unwrap()),
            },
            FnCall {
                sig: 0xabc1234567890def,
                name: CString::new("bar").unwrap(),
                debug_param_types: vec![CString::new("bar").unwrap(), CString::new("baz").unwrap()],
                debug_return_type: Some(CString::new("quxxx").unwrap()),
            },
        ];

        let mut buf = Vec::new();
        expect.iter().for_each(|meta| {
            buf.extend(meta.to_bytes());
        });

        let result = FnCall::try_from_bytes_vec(buf.as_slice(), true).unwrap();
        assert_eq!(expect, result);
    }

    #[cfg(feature = "vmi-consume")]
    #[test]
    fn from_bytes_vec_debug_partial() {
        let mut buf = Vec::new();
        buf.extend(
            FnCall {
                sig: 0x1234567890abcdef,
                name: CString::new("foo").unwrap(),
                debug_param_types: vec![CString::new("bar").unwrap()],
                debug_return_type: None,
            }
            .to_bytes(),
        );
        buf.extend_from_slice(b"invalid");

        let result = FnCall::try_from_bytes_vec(buf.as_slice(), true);
        assert!(matches!(result, Err(_)));
    }
}
