use crate::mem::{AlignedNonZeroUsize, VirtAddr};
use crate::typesignature::TypeSignature;
use core::alloc::{Allocator, Layout};
use core::fmt::{LowerHex, UpperHex};
use core::mem::ManuallyDrop;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use spin::once::Once;
use talc::{ErrOnOom, Span, Talc};

static ALLOC_FOREIGN: Once<AllocImpl<spin::Mutex<()>, ErrOnOom>> = Once::new();
static ALLOC_OWN: Once<AllocImpl<spin::Mutex<()>, ErrOnOom>> = Once::new();

#[cfg_attr(
    feature = "vmi-consume",
    derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)
)]
pub enum Error {
    #[cfg_attr(feature = "vmi-consume", error("allocator is not initialized"))]
    UninitializedAllocator,
    #[cfg_attr(feature = "vmi-consume", error("Pointer is null"))]
    NullPointer,
    #[cfg_attr(feature = "vmi-consume", error("Out of memory"))]
    OutOfMemory,
    #[cfg_attr(
        feature = "vmi-consume",
        error("Not enough space to initialize allocator")
    )]
    InitNotEnoughSpace,
    #[cfg_attr(
        feature = "vmi-consume",
        error("Invalid offset pointer (out of bounds)")
    )]
    InvalidOffsetPtr,
}

struct AllocImpl<M: lock_api::RawMutex, O: talc::OomHandler> {
    talck: talc::Talck<M, O>,
    base: VirtAddr,
    capacity: usize,
}

impl<M: lock_api::RawMutex, O: talc::OomHandler> AllocImpl<M, O> {
    fn new(oom: O, arena: Arena) -> Result<Self, Error> {
        let mut talc = Talc::new(oom);
        let span = unsafe {
            talc.claim(arena.into())
                .map_err(|_| Error::InitNotEnoughSpace)?
        };
        let talck = talc.lock::<M>();

        let (b, _) = span.get_base_acme().unwrap();
        let base = VirtAddr::from_ptr(b);
        let capacity = span.size();
        Ok(Self {
            talck,
            base,
            capacity,
        })
    }

    unsafe fn alloc<T: TypeSignature>(&self) -> Result<Owned<T>, Error> {
        let layout = Layout::new::<T>();
        self.talck
            .allocate(layout)
            .map(|ptr| Owned {
                inner: ptr.cast::<T>(),
            })
            .map_err(|_| Error::OutOfMemory)
    }

    unsafe fn alloc_buf(&self, size: usize) -> Result<OwnedBuf, Error> {
        let align = align_of::<u8>();
        let layout = Layout::from_size_align(size, align).unwrap();

        let ptr = self
            .talck
            .allocate(layout)
            .map(|ptr| ptr.cast::<u8>())
            .map_err(|_| Error::OutOfMemory)?;

        Ok(OwnedBuf::new(ptr, NonZeroUsize::new(size).unwrap()))
    }

    fn dealloc<T: TypeSignature>(&self, ptr: NonNull<T>) {
        let layout = Layout::new::<T>();
        unsafe { self.talck.deallocate(ptr.cast::<u8>(), layout) }
    }

    fn dealloc_buf(&self, ptr: NonNull<u8>, capacity: NonZeroUsize) {
        let align = align_of::<u8>();
        let layout = Layout::from_size_align(capacity.get(), align).unwrap();
        unsafe { self.talck.deallocate(ptr, layout) }
    }

    /// Check the offset pointer for validity (fits in the arena) and return a readable reference
    /// to the underlying data.
    fn get_foreign<T: TypeSignature>(&self, offset: OffsetPtr<T>) -> Result<Foreign<T>, Error> {
        if offset.offset as usize + size_of::<T>() > self.capacity {
            return Err(Error::InvalidOffsetPtr);
        }

        // construct NonNull<T> purely for null pointer checks
        // Result is not needed later on, as NonNull does not impl Send, it can not be used
        // in VMI related structs
        // TODO: check if this check is even necessary
        let addr = self.base + offset.offset as u64;
        let ptr = addr.as_ptr::<T>().cast_mut();
        let _ = NonNull::new(ptr).ok_or(Error::NullPointer)?;

        Ok(Foreign { ptr: offset })
    }

    fn get<T: TypeSignature>(&self, ptr: &OffsetPtr<T>) -> &T {
        let addr = self.base + ptr.offset as u64;
        let value_ptr: *const T = addr.as_ptr::<T>().cast();
        unsafe { value_ptr.as_ref().unwrap() }
    }

    fn get_ptr<T: Unpackable>(&self, ptr: &OffsetPtr<T>) -> *const T {
        let addr = self.base + ptr.offset as u64;
        addr.as_ptr::<T>().cast()
    }

    /// Transform a Foreign<T> to a NonNull<T>
    fn get_non_null<T: TypeSignature>(&self, offset_ptr: &OffsetPtr<T>) -> NonNull<T> {
        let addr = self.base + offset_ptr.offset as u64;
        let ptr = addr.as_ptr::<T>().cast_mut();
        // SAFETY: can be unchecked, as Foreign<T> was constructed by this allocator and null ptr
        // check was done on construction
        unsafe { NonNull::new_unchecked(ptr) }
    }

    /// Transform a NonNull<T> to an OffsetPtr<T>
    pub fn ptr_offset<T: TypeSignature>(&self, ptr: NonNull<T>) -> OffsetPtr<T> {
        let offset = ptr.as_ptr() as u64 - self.base.as_u64();
        OffsetPtr::from(offset as u32)
    }
}

pub struct Arena {
    pub ptr: NonNull<u8>,
    pub capacity: AlignedNonZeroUsize,
}

impl Arena {
    pub fn new(ptr: NonNull<u8>, capacity: AlignedNonZeroUsize) -> Self {
        Self { ptr, capacity }
    }
}

impl Into<Span> for Arena {
    fn into(self) -> Span {
        Span::from_base_size(self.ptr.as_ptr(), self.capacity.get())
    }
}

pub fn init(owning: Arena, foreign: Arena) {
    ALLOC_OWN.call_once(|| match AllocImpl::new(ErrOnOom, owning) {
        Ok(alloc) => alloc,
        Err(_) => panic!("Failed to initialize allocator"),
    });

    ALLOC_FOREIGN.call_once(|| match AllocImpl::new(ErrOnOom, foreign) {
        Ok(alloc) => alloc,
        Err(_) => panic!("Failed to initialize allocator"),
    });
}

/// Allocate type T on the shared memory. This should only be used for data destined for the
/// remote peer. The peer will free the allocated memory if the data is dropped. The original
/// allocator can also drop it, but should only be done if one can ensure that the peer will not
/// use the memory anymore.
pub unsafe fn alloc<T: TypeSignature>() -> Result<Owned<T>, Error> {
    unsafe {
        match ALLOC_OWN.get() {
            Some(alloc) => alloc.alloc(),
            None => Err(Error::UninitializedAllocator),
        }
    }
}

/// Allocate an owned buffer of the given size. This should only be used for data destined for the
/// remote peer. The peer will free the allocated memory if the data is dropped. The original
/// allocator can also drop it, but should only be done if one can ensure that the peer will not
/// use the memory anymore.
pub unsafe fn alloc_buf(size: usize) -> Result<OwnedBuf, Error> {
    unsafe {
        match ALLOC_OWN.get() {
            Some(alloc) => alloc.alloc_buf(size),
            None => Err(Error::UninitializedAllocator),
        }
    }
}

/// Deallocate a type allocated by `alloc`. Make sure to only call this if one can ensure that the
/// peer will not use the memory anymore.
pub fn dealloc<T: TypeSignature>(ptr: NonNull<T>) {
    match ALLOC_OWN.get() {
        Some(alloc) => alloc.dealloc(ptr),
        None => return,
    }
}

/// Deallocate a buffer allocated by `alloc_buf`. Make sure to only call this if one can ensure that the
/// peer will not use the memory anymore.
pub fn dealloc_buf(buf: OwnedBuf) {
    match ALLOC_OWN.get() {
        Some(alloc) => alloc.dealloc_buf(buf.ptr, buf.capacity),
        None => return,
    }
}

pub unsafe fn get_foreign<T: TypeSignature>(ptr: OffsetPtr<T>) -> Result<Foreign<T>, Error> {
    match ALLOC_FOREIGN.get() {
        Some(alloc) => alloc.get_foreign(ptr),
        None => Err(Error::UninitializedAllocator),
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RawOffsetPtr {
    inner: u32,
}

impl LowerHex for RawOffsetPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.inner, f)
    }
}

impl UpperHex for RawOffsetPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.inner, f)
    }
}

#[cfg(feature = "vmi-consume")]
impl core::fmt::Display for RawOffsetPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl RawOffsetPtr {
    pub const fn as_u32(self) -> u32 {
        self.inner
    }
}

impl From<u32> for RawOffsetPtr {
    fn from(value: u32) -> Self {
        Self { inner: value }
    }
}

#[repr(transparent)]
pub struct OffsetPtr<T: TypeSignature> {
    pub offset: u32,
    _marker: core::marker::PhantomData<T>,
}

impl<T: TypeSignature> From<u32> for OffsetPtr<T> {
    fn from(value: u32) -> Self {
        Self {
            offset: value,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: TypeSignature> From<RawOffsetPtr> for OffsetPtr<T> {
    fn from(ptr: RawOffsetPtr) -> Self {
        Self::from(ptr.inner)
    }
}

impl<T: TypeSignature> TypeSignature for OffsetPtr<T> {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"OffsetPtr");
        h.write(<T as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from(format!("OffsetPtr<{}>", T::name()))
    }
}

/// Owned allocation for future sharing with the VMI peer.
#[repr(transparent)]
pub struct Owned<T: TypeSignature> {
    inner: NonNull<T>,
}

impl<T: TypeSignature> Owned<T> {
    pub fn into_shared(self) -> Shared<T> {
        let alloc = ALLOC_OWN.get().unwrap();
        Shared {
            inner: alloc.ptr_offset(self.inner),
        }
    }
}

impl<T: TypeSignature> AsRef<T> for Owned<T> {
    fn as_ref(&self) -> &T {
        unsafe { self.inner.as_ref() }
    }
}

impl<T: TypeSignature> AsMut<T> for Owned<T> {
    fn as_mut(&mut self) -> &mut T {
        unsafe { self.inner.as_mut() }
    }
}

#[repr(transparent)]
pub struct Shared<T: TypeSignature> {
    pub(crate) inner: OffsetPtr<T>,
}

impl<T: TypeSignature> From<Owned<T>> for Shared<T> {
    fn from(owned: Owned<T>) -> Self {
        let alloc = ALLOC_OWN.get().unwrap();
        Shared {
            inner: alloc.ptr_offset(owned.inner),
        }
    }
}

impl<T: TypeSignature> TypeSignature for Shared<T> {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"Shared");
        h.write(<T as TypeSignature>::SIGNATURE.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from(format!("Shared<{}>", T::name()))
    }
}

/// Owned buffer allocated for future sharing with the VMI peer.
/// VMI messages attributes should use `SharedBuf` instead of `OwnedBuf` to hint on a
/// type-level that the receiving peer should not mutate the underlying data.
#[repr(C)]
pub struct OwnedBuf {
    ptr: NonNull<u8>,
    capacity: NonZeroUsize,
}

impl OwnedBuf {
    fn new(ptr: NonNull<u8>, capacity: NonZeroUsize) -> Self {
        Self { ptr, capacity }
    }

    pub fn into_shared(self) -> SharedBuf {
        let alloc = ALLOC_OWN.get().unwrap();
        let offset = alloc.ptr_offset(self.ptr);

        SharedBuf {
            ptr: offset,
            capacity: self.capacity,
        }
    }
}

impl AsRef<[u8]> for OwnedBuf {
    fn as_ref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.capacity.get()) }
    }
}

impl AsMut<[u8]> for OwnedBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity.get()) }
    }
}

/// Shared buffer allocated for sharing with the VMI peer.
#[repr(C)]
pub struct SharedBuf {
    pub(crate) ptr: OffsetPtr<u8>,
    pub(crate) capacity: NonZeroUsize,
}

impl SharedBuf {
    /// This function deallocates the buffer.
    /// SAFETY: using the value after this function call triggers undefined behavior! This extends
    /// to usage by the VMI peer!
    pub fn deallocate(self) {
        // unwrap is safe because the allocator is needed to even construct the foreign pointer
        let alloc = ALLOC_OWN.get().unwrap();
        let ptr = alloc.get_non_null(&self.ptr);
        alloc.dealloc_buf(ptr, self.capacity);
    }
}

impl TypeSignature for SharedBuf {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"SharedBuf");
        h.write(
            <OffsetPtr<u8> as TypeSignature>::SIGNATURE
                .to_le_bytes()
                .as_slice(),
        );
        h.write(1u64.to_le_bytes().as_slice());
        h.write(
            <NonZeroUsize as TypeSignature>::SIGNATURE
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from("SharedBuf")
    }
}

/// Foreign memory allocated by the VMI peer.
/// This wraps a raw pointer and manages deallocation on drop.
#[repr(transparent)]
pub struct Foreign<T: TypeSignature> {
    ptr: OffsetPtr<T>,
}

impl<T: TypeSignature> Foreign<T> {
    /// Get the underlying value of the pointer.
    pub fn get(&self) -> &T {
        let alloc = ALLOC_FOREIGN.get().unwrap();
        alloc.get(&self.ptr)
    }
}

impl<T: Unpackable> Foreign<T> {
    pub fn get_ptr(&self) -> *const T {
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_ptr(&self.ptr);
        ptr
    }

    pub unsafe fn unpack(self) -> T::Output {
        // ManuallyDrop to prevent automatic dropping
        let this = ManuallyDrop::new(self);
        // get the raw pointer to the underlying value
        let ptr = this.get_ptr();
        // unpack the fields from the struct (copies the values)
        let output = unsafe { T::unpack(ptr) };
        // due to the copy of the underlying struct fields, the original value can be deallocated
        // without dropping (prevents double drop)
        this.manually_dealloc();

        output
    }

    /// This function triggers a deallocation without drop of the underlying pointer.
    /// SAFETY: using the value after this function call triggers undefined behaviour.
    pub fn manually_dealloc(&self) {
        // unwrap is safe because the allocator is needed to even construct the foreign pointer
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_non_null(&self.ptr);
        alloc.dealloc(ptr);
    }
}

impl<T: TypeSignature> TypeSignature for Foreign<T> {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"Foreign");
        h.write(
            <OffsetPtr<T> as TypeSignature>::SIGNATURE
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from(format!("Foreign<{}>", T::name()))
    }
}

impl<T: TypeSignature> TypeSignature for &Foreign<T> {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::from_partial(Foreign::<T>::SIGNATURE);
        h.write(b"&Foreign");
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;

    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from(format!("&Foreign<{}>", T::name()))
    }
}

impl<T: TypeSignature> Drop for Foreign<T> {
    fn drop(&mut self) {
        // unwrap is safe because the allocator is needed to even construct the foreign pointer
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_non_null(&self.ptr);
        alloc.dealloc(ptr);
    }
}

/// Foreign buffer allocated by the VMI peer.
pub struct ForeignBuf {
    pub(crate) ptr: Foreign<u8>,
    pub(crate) capacity: NonZeroUsize,
}

impl AsRef<[u8]> for ForeignBuf {
    fn as_ref(&self) -> &[u8] {
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_non_null(&self.ptr.ptr);
        unsafe { core::slice::from_raw_parts(ptr.as_ptr(), self.capacity.get()) }
    }
}

impl TypeSignature for ForeignBuf {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"ForeignBuf");
        h.write(
            <OffsetPtr<u8> as TypeSignature>::SIGNATURE
                .to_le_bytes()
                .as_slice(),
        );
        h.write(1u64.to_le_bytes().as_slice());
        h.write(
            <NonZeroUsize as TypeSignature>::SIGNATURE
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from("ForeignBuf")
    }
}

impl TypeSignature for &ForeignBuf {
    const SIGNATURE: u64 = {
        let mut h = crate::hash::SignatureHasher::from_partial(ForeignBuf::SIGNATURE);
        h.write(b"&ForeignBuf");
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
    #[cfg(feature = "vmi-consume")]
    fn name() -> String {
        String::from("&ForeignBuf")
    }
}

/// This marks a type as being only there for the VMI parameter/return type transport, and should
/// NEVER be implemented manually.
pub unsafe trait Unpackable: TypeSignature {
    type Output;
    /// unpack copies the struct values via ptr::read
    unsafe fn unpack(this: *const Self) -> Self::Output;
}
