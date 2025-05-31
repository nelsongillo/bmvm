use x86_64::PhysAddr;
use x86_64::structures::paging::{FrameAllocator, FrameDeallocator, PageSize, PhysFrame};


// #[call_host]
unsafe extern "C" {
    fn alloc(size: u64) -> u64;
    fn dealloc(ptr: u64);
}

struct Allocator {}

unsafe impl<S: PageSize> FrameAllocator<S> for Allocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        unsafe {
            let ptr = alloc(S::SIZE);
            Some(PhysFrame::from_start_address(PhysAddr::new(ptr)).unwrap())
        }
    }
}

impl<S: PageSize> FrameDeallocator<S> for Allocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<S>) {
        unsafe {
            dealloc(frame.start_address().as_u64());
        }
    }
}