mod layout;
mod align;
mod addr;

pub use addr::*;
pub use align::*;
pub use layout::*;


#[inline]
pub fn aligned_and_fits<A: Align>(from: u64, to: u64) -> bool {
    A::is_aligned(from) && to - from >= A::ALIGNMENT
}
