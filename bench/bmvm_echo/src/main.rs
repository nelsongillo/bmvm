#![no_std]
#![no_main]

use bmvm_guest::{ForeignBuf, SharedBuf, alloc_buf, expose};

#[expose]
fn noop() {
    // unsafe {
    //     core::arch::asm!(
    //         "add rsp, 8",  // Compensate for the push
    //     )
    // };
}

#[expose]
fn reverse(foreign: ForeignBuf) -> SharedBuf {
    let mut owned = unsafe { alloc_buf(foreign.len()).ok().unwrap() };
    let buf = owned.as_mut();

    buf.copy_from_slice(foreign.as_ref());
    let half_len = buf.len() / 2;
    let core::ops::Range { start, end } = buf.as_mut_ptr_range();
    let (front_half, back_half) = unsafe {
        (
            core::slice::from_raw_parts_mut(start, half_len),
            core::slice::from_raw_parts_mut(end.sub(half_len), half_len),
        )
    };
    revswap(front_half, back_half, half_len);
    #[inline]
    const fn revswap<T>(a: &mut [T], b: &mut [T], n: usize) {
        core::debug_assert!(a.len() == n);
        core::debug_assert!(b.len() == n);
        let (a, _) = a.split_at_mut(n);
        let (b, _) = b.split_at_mut(n);

        let mut i = 0;
        while i < n {
            core::mem::swap(&mut a[i], &mut b[n - 1 - i]);
            i += 1;
        }
    }

    owned.into_shared()
}
