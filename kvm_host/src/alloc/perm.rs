#![allow(unused)]

/// Perm represents a generic permission
pub trait Perm {}

/// Writeable represents a write permission
pub trait Writable: Perm {}

/// Readable represents a read permission
pub trait Readable: Perm {}

/// Anon represents no granted permission
pub trait Anon: Perm {}

/// ReadOnly implements the Readable trait. This should be used as the generic.
pub struct ReadOnly;

impl Perm for ReadOnly {}

impl Readable for ReadOnly {}
/// WriteOnly implements the Writeable trait. This should be used as the generic.
pub struct WriteOnly;
impl Perm for WriteOnly {}

impl Writable for WriteOnly {}
/// ReadWrite implements the Writeable, as well as the Readable trait. This should be used as the generic.
pub struct ReadWrite;
impl Perm for ReadWrite {}

impl Readable for ReadWrite {}
impl Writable for ReadWrite {}

/// GuestOnly implements Anon trait, indication neither a read nor a write permission is granted.
pub struct GuestOnly;

impl Perm for GuestOnly {}

impl Anon for GuestOnly {}
