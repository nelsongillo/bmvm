#![allow(unused)]

use nix::sys::mman::ProtFlags;
use sealed::sealed;

/// Perm represents a generic permission
#[sealed]
pub trait Perm {
    fn prot_flags() -> ProtFlags;
}

#[sealed]
pub trait Accessible {}

/// ReadOnly implements the Readable trait. This should be used as the generic.
pub struct ReadOnly;

#[sealed]
impl Perm for ReadOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_READ
    }
}

#[sealed]
impl Accessible for ReadOnly {}

/// WriteOnly implements the Writeable trait. This should be used as the generic.
pub struct WriteOnly;

#[sealed]
impl Perm for WriteOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_WRITE
    }
}

#[sealed]
impl Accessible for WriteOnly {}

/// ReadWrite implements the Writeable, as well as the Readable trait. This should be used as the generic.
pub struct ReadWrite;

#[sealed]
impl Perm for ReadWrite {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_WRITE | ProtFlags::PROT_READ
    }
}

#[sealed]
impl Accessible for ReadWrite {}

/// GuestOnly implements Anon trait, indication neither a read nor a write permission is granted.
/// We try to create a guest-only region via the `KVM_CREATE_GUEST_MEMFD` ioctl. If the capability
/// is not available, the fallback ReadWrite will be used.
pub struct GuestOnly;

#[sealed]
impl Perm for GuestOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_NONE
    }
}
