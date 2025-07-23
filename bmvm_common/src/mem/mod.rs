mod addr;
mod align;
mod alloc;
mod bits;
mod layout;

pub use addr::*;
pub use align::*;
pub use alloc::*;
pub use bits::*;
pub use layout::*;

#[inline]
pub fn aligned_and_fits<A: Align>(from: u64, to: u64) -> bool {
    if to < from {
        return false;
    }

    A::is_aligned(from) && to - from >= A::ALIGNMENT
}

mod tests {
    #![allow(unused_imports)]
    use crate::mem::{Page4KiB, aligned_and_fits};

    #[test]
    fn not_aligned() {
        assert!(!aligned_and_fits::<Page4KiB>(0x1001, 0x2000));
    }

    #[test]
    fn aligned_but_not_enough_space() {
        assert!(!aligned_and_fits::<Page4KiB>(0x1000, 0x1FFF));
    }

    #[test]
    fn fits_exactly() {
        assert!(aligned_and_fits::<Page4KiB>(0x1000, 0x2000));
    }

    #[test]
    fn more_than_fits() {
        assert!(aligned_and_fits::<Page4KiB>(0x1000, 0x10000));
    }

    #[test]
    fn from_bigger_than_to() {
        assert!(!aligned_and_fits::<Page4KiB>(0x1000, 0x001));
    }

    #[test]
    fn from_eq_to() {
        assert!(!aligned_and_fits::<Page4KiB>(0x1000, 0x1000));
    }
}
