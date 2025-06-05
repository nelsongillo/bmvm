use core::fmt::{Debug, Display};
#[cfg(feature = "std")]
use std::fmt::{Formatter};

pub trait Zero {
    /// zero sets all values to their respective 0 value
    fn zero(&mut self);
}

pub enum InterpretError {
    TooSmall(usize, usize),
    Misaligned(usize, usize),
}

#[cfg(feature = "std")]
impl Debug for InterpretError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            InterpretError::TooSmall(want, got) => {
                write!(
                    f,
                    "provided slice was to small: expected {} but got {}",
                    want, got
                )
            }
            InterpretError::Misaligned(want, got) => {
                write!(f, "misaligned pointer: expected {} but got {}", want, got)
            }
        }
    }
}

#[cfg(feature = "std")]
impl Display for InterpretError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            InterpretError::TooSmall(want, got) => {
                write!(
                    f,
                    "provided slice was to small: expected {} but got {}",
                    want, got
                )
            }
            InterpretError::Misaligned(want, got) => {
                write!(f, "misaligned pointer: expected {} but got {}", want, got)
            }
        }
    }
}

#[cfg(feature = "std")]
impl core::error::Error for InterpretError {}

pub trait Interpret: Sized {
    /// Interpret a byte buffer as a struct.
    fn from_bytes(buf: &[u8]) -> Result<&Self, InterpretError> {
        is_aligned::<Self>(buf)?;
        fits::<Self>(buf)?;
        Ok(unsafe { &*(buf.as_ptr() as *const Self) })
    }

    /// Interpret a mutable byte buffer as a mutable struct.
    fn from_mut_bytes(buf: &mut [u8]) -> Result<&mut Self, InterpretError> {
        is_aligned::<Self>(buf)?;
        fits::<Self>(buf)?;

        Ok(unsafe { &mut *(buf.as_mut_ptr() as *mut Self) })
    }
}

/// Check if the buffer is aligned to be properly interpreted as T
fn is_aligned<T>(buf: &[u8]) -> Result<(), InterpretError> {
    if ((buf.as_ptr() as usize) % align_of::<T>()) != 0 {
        Err(InterpretError::Misaligned(
            align_of::<T>(),
            buf.as_ptr() as usize,
        ))
    } else {
        Ok(())
    }
}

/// Check if T can even fit into the provided slice
fn fits<T>(buf: &[u8]) -> Result<(), InterpretError> {
    if buf.len() < size_of::<T>() {
        Err(InterpretError::TooSmall(size_of::<T>(), buf.len()))
    } else {
        Ok(())
    }
}

mod test {
    #![allow(unused)]

    use super::*;

    #[repr(C, packed)]
    struct Dummy {
        foo: u8,
        bar: u16,
        baz: u8,
    }

    impl Zero for Dummy {
        fn zero(&mut self) {
            self.foo = 0;
            self.bar = 0;
            self.baz = 0;
        }
    }

    impl Interpret for Dummy {}

    #[test]
    fn interpret_from_mut_bytes() {
        let mut buf = [0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8];
        let d = match Dummy::from_mut_bytes(buf.as_mut_slice()) {
            Ok(d) => d,
            Err(_) => unreachable!(),
        };

        assert_eq!(0x1, d.foo);
        let bar = d.bar;
        assert_eq!(0x0302, bar);
        assert_eq!(0x04, d.baz);

        d.baz = 0xff;
        assert_eq!([0x1, 0x2, 0x3, 0xff, 0x5, 0x6, 0x7, 0x8], buf);
    }

    #[test]
    fn create_at_buffer_too_small() {
        let mut buf = [0x1, 0x2, 0x3];
        match Dummy::from_mut_bytes(buf.as_mut_slice()) {
            Ok(_) => unreachable!(),
            Err(_) => (),
        }
    }
}
