#![allow(unused)]

use nix::libc::PROT_READ;
use nix::sys::mman::ProtFlags;

/// Perm represents a generic permission
pub trait Perm {
    fn prot_flags() -> ProtFlags;
}

/// Writeable represents a write permission
pub trait Writable: Perm {}

/// Readable represents a read permission
pub trait Readable: Perm {}

/// Anon represents no granted permission
pub trait Anon: Perm {}

/// ReadOnly implements the Readable trait. This should be used as the generic.
pub struct ReadOnly;

impl Perm for ReadOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_READ
    }
}

impl Readable for ReadOnly {}
/// WriteOnly implements the Writeable trait. This should be used as the generic.
pub struct WriteOnly;
impl Perm for WriteOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_WRITE
    }
}

impl Writable for WriteOnly {}
/// ReadWrite implements the Writeable, as well as the Readable trait. This should be used as the generic.
pub struct ReadWrite;
impl Perm for ReadWrite {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_WRITE | ProtFlags::PROT_READ
    }
}

impl Readable for ReadWrite {}
impl Writable for ReadWrite {}

/// GuestOnly implements Anon trait, indication neither a read nor a write permission is granted.
/// We try to create a guest-only region via the `KVM_CREATE_GUEST_MEMFD` ioctl. If the capability
/// is not available, the fallback ReadWrite will be used.
pub struct GuestOnly;

impl Perm for GuestOnly {
    #[inline]
    fn prot_flags() -> ProtFlags {
        ProtFlags::PROT_NONE
    }
}

impl Anon for GuestOnly {}
