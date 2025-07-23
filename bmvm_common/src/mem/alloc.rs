use crate::TypeHash;
use crate::mem::{AlignedNonZeroUsize, VirtAddr};
use core::alloc::{Allocator, Layout};
use core::arch::asm;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use spin::once::Once;
use talc::{ErrOnOom, Span, Talc};

#[sealed::sealed]
pub unsafe trait OwnedShareable {
    unsafe fn write(&self);
}

#[sealed::sealed]
pub trait ForeignShareable {}

#[sealed::sealed]
unsafe impl OwnedShareable for () {
    unsafe fn write(&self) {}
}

macro_rules! impl_shareable_for_primitives {
    ($($prim:ty),* $(,)?) => {
        $(
            #[sealed::sealed]
            unsafe impl OwnedShareable for $prim {
                unsafe fn write(&self) {
                    unsafe {
                        asm!(
                            "mov {0}, rbx",
                            in(reg) *self as u64,
                            options(nostack, preserves_flags)
                        )
                    }
                }
            }
        )*
    };
}
impl_shareable_for_primitives!(
    u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, bool, char, usize
);

static ALLOC_FOREIGN: Once<AllocImpl<spin::Mutex<()>, ErrOnOom>> = Once::new();
static ALLOC_OWN: Once<AllocImpl<spin::Mutex<()>, ErrOnOom>> = Once::new();

pub enum Error {
    UninitializedAllocator,
    NullPointer,
    OutOfMemory,
    NotEnoughSpace,
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
                .map_err(|_| Error::NotEnoughSpace)?
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

    unsafe fn alloc<T: TypeHash>(&self) -> Result<Owned<T>, Error> {
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

    fn dealloc<T: TypeHash>(&self, ptr: NonNull<T>) {
        let layout = Layout::new::<T>();
        unsafe { self.talck.deallocate(ptr.cast::<u8>(), layout) }
    }

    fn dealloc_buf(&self, buf: OwnedBuf) {
        let align = align_of::<u8>();
        let layout = Layout::from_size_align(buf.capacity.get(), align).unwrap();
        unsafe { self.talck.deallocate(buf.ptr, layout) }
    }

    /// Check the offset pointer for validity (fits in the arena) and return a readable reference
    /// to the underlying data.
    fn get_foreign<T: TypeHash>(&self, offset: OffsetPtr<T>) -> Result<Foreign<T>, Error> {
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

    fn get<T: TypeHash>(&self, ptr: &OffsetPtr<T>) -> &T {
        let addr = self.base + ptr.offset as u64;
        let ptr = addr.as_ptr::<T>().cast_mut();
        unsafe { ptr.as_ref().unwrap() }
    }

    /// Transform a Foreign<T> to a NonNull<T>
    fn get_non_null<T: TypeHash>(&self, foreign: &Foreign<T>) -> NonNull<T> {
        let addr = self.base + foreign.ptr.offset as u64;
        let ptr = addr.as_ptr::<T>().cast_mut();
        // SAFETY: can be unchecked, as Foreign<T> was constructed by this allocator and null ptr
        // check was done on construction
        unsafe { NonNull::new_unchecked(ptr) }
    }

    /// Transform a NonNull<T> to an OffsetPtr<T>
    pub fn ptr_offset<T: TypeHash>(&self, ptr: NonNull<T>) -> OffsetPtr<T> {
        let offset = ptr.as_ptr() as u64 - self.base.as_u64();
        OffsetPtr::from(offset as u32)
    }
}

pub struct Arena {
    pub ptr: NonNull<u8>,
    pub capacity: AlignedNonZeroUsize,
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
pub unsafe fn alloc<T: TypeHash>() -> Result<Owned<T>, Error> {
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
pub fn dealloc<T: TypeHash>(ptr: NonNull<T>) {
    match ALLOC_OWN.get() {
        Some(alloc) => alloc.dealloc(ptr),
        None => return,
    }
}

/// Deallocate a buffer allocated by `alloc_buf`. Make sure to only call this if one can ensure that the
/// peer will not use the memory anymore.
pub fn dealloc_buf(buf: OwnedBuf) {
    match ALLOC_OWN.get() {
        Some(alloc) => alloc.dealloc_buf(buf),
        None => return,
    }
}

pub unsafe fn get_foreign<T: TypeHash>(ptr: OffsetPtr<T>) -> Result<Foreign<T>, Error> {
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
pub struct OffsetPtr<T: TypeHash> {
    pub offset: u32,
    _marker: core::marker::PhantomData<T>,
}

impl<T: TypeHash> From<u32> for OffsetPtr<T> {
    fn from(value: u32) -> Self {
        Self {
            offset: value,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: TypeHash> From<RawOffsetPtr> for OffsetPtr<T> {
    fn from(ptr: RawOffsetPtr) -> Self {
        Self::from(ptr.inner)
    }
}

impl<T: TypeHash> TypeHash for OffsetPtr<T> {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"OffsetPtr");
        h.write(<T as TypeHash>::TYPE_HASH.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

/// Owned allocation for future sharing with the VMI peer.
pub struct Owned<T: TypeHash> {
    inner: NonNull<T>,
}

impl<T: TypeHash> AsRef<T> for Owned<T> {
    fn as_ref(&self) -> &T {
        unsafe { self.inner.as_ref() }
    }
}

impl<T: TypeHash> AsMut<T> for Owned<T> {
    fn as_mut(&mut self) -> &mut T {
        unsafe { self.inner.as_mut() }
    }
}

impl<T: TypeHash> Into<Shared<T>> for Owned<T> {
    fn into(self) -> Shared<T> {
        let alloc = ALLOC_OWN.get().unwrap();
        Shared {
            inner: alloc.ptr_offset(self.inner),
        }
    }
}

#[repr(transparent)]
pub struct Shared<T: TypeHash> {
    inner: OffsetPtr<T>,
}

impl<T: TypeHash> TypeHash for Shared<T> {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"Shared");
        h.write(<T as TypeHash>::TYPE_HASH.to_le_bytes().as_slice());
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

#[sealed::sealed]
unsafe impl<T: TypeHash> OwnedShareable for Shared<T> {
    #[inline(always)]
    unsafe fn write(&self) {
        unsafe {
            asm!(
            "mov {0:e}, ebx",
            in(reg) self.inner.offset,
            options(nostack, preserves_flags)
            )
        }
    }
}

/// Owned buffer allocated for future sharing with the VMI peer.
/// VMI messages attributes should use `SharedBuf` instead of `OwnedBuf` to hint on a
/// type-level that the receiving peer should not mutate the underlying data.
pub struct OwnedBuf {
    ptr: NonNull<u8>,
    capacity: NonZeroUsize,
}

impl OwnedBuf {
    fn new(ptr: NonNull<u8>, capacity: NonZeroUsize) -> Self {
        Self { ptr, capacity }
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

impl Into<SharedBuf> for OwnedBuf {
    fn into(self) -> SharedBuf {
        let alloc = ALLOC_OWN.get().unwrap();
        let offset = alloc.ptr_offset(self.ptr.cast());

        SharedBuf {
            ptr: offset,
            capacity: self.capacity,
        }
    }
}

/// Shared buffer allocated for sharing with the VMI peer.
pub struct SharedBuf {
    ptr: OffsetPtr<u8>,
    capacity: NonZeroUsize,
}

impl TypeHash for SharedBuf {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"SharedBuf");
        h.write(
            <OffsetPtr<u8> as TypeHash>::TYPE_HASH
                .to_le_bytes()
                .as_slice(),
        );
        h.write(1u64.to_le_bytes().as_slice());
        h.write(
            <NonZeroUsize as TypeHash>::TYPE_HASH
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

#[sealed::sealed]
unsafe impl OwnedShareable for SharedBuf {
    #[inline(always)]
    unsafe fn write(&self) {
        unsafe {
            asm!(
                "mov {0:e}, ebx",
                "mov {1}, rcx",
                in(reg) self.ptr.offset,
                in(reg) self.capacity.get(),
                options(nostack, preserves_flags)
            )
        }
    }
}

/// Foreign memory allocated by the VMI peer.
/// This wraps a raw pointer and manages deallocation on drop.
pub struct Foreign<T: TypeHash> {
    ptr: OffsetPtr<T>,
}

impl<T: TypeHash> Foreign<T> {
    /// Get the underlying value of the pointer.
    pub fn get(&self) -> &T {
        ALLOC_FOREIGN.get().unwrap().get(&self.ptr)
    }
}

impl<T: TypeHash> TypeHash for Foreign<T> {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"Foreign");
        h.write(
            <OffsetPtr<T> as TypeHash>::TYPE_HASH
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

impl<T: TypeHash> TypeHash for &Foreign<T> {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::from_partial(Foreign::<T>::TYPE_HASH);
        h.write(b"&Foreign");
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

#[sealed::sealed]
impl<T: TypeHash> ForeignShareable for Foreign<T> {}

impl<T: TypeHash> Drop for Foreign<T> {
    fn drop(&mut self) {
        // unwrap is safe because the allocator is needed to even construct the foreign pointer
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_non_null(self);
        alloc.dealloc(ptr);
    }
}

/// Foreign buffer allocated by the VMI peer.
pub struct ForeignBuf {
    ptr: Foreign<u8>,
    capacity: NonZeroUsize,
}

impl AsRef<[u8]> for ForeignBuf {
    fn as_ref(&self) -> &[u8] {
        let alloc = ALLOC_FOREIGN.get().unwrap();
        let ptr = alloc.get_non_null(&self.ptr);
        unsafe { core::slice::from_raw_parts(ptr.as_ptr(), self.capacity.get()) }
    }
}

impl TypeHash for ForeignBuf {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::new();
        h.write(0u64.to_le_bytes().as_slice());
        h.write(b"ForeignBuf");
        h.write(
            <OffsetPtr<u8> as TypeHash>::TYPE_HASH
                .to_le_bytes()
                .as_slice(),
        );
        h.write(1u64.to_le_bytes().as_slice());
        h.write(
            <NonZeroUsize as TypeHash>::TYPE_HASH
                .to_le_bytes()
                .as_slice(),
        );
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

impl TypeHash for &ForeignBuf {
    const TYPE_HASH: u64 = {
        let mut h = crate::hash::Djb2::from_partial(ForeignBuf::TYPE_HASH);
        h.write(b"&ForeignBuf");
        h.finish()
    };
    const IS_PRIMITIVE: bool = false;
}

#[sealed::sealed]
impl ForeignShareable for ForeignBuf {}
