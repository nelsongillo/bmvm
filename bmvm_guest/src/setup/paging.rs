use crate::setup::{gdt, idt};
use bmvm_common::error::ExitCode;
use bmvm_common::mem::*;
use core::ops::Add;
use x86_64::structures::paging::mapper::PageTableFrameMapping;
use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator, MappedPageTable, Mapper, PageSize, PageTable, PageTableFlags,
    PhysFrame, Size1GiB, Size2MiB, Size4KiB,
};

pub(crate) fn setup(table: &LayoutTable, sys: LayoutTableEntry) -> Result<(), ExitCode> {
    let pml4 = unsafe { &mut *(sys.addr().as_u64() as *mut PageTable) };
    let mut mapper = unsafe { MappedPageTable::new(pml4, Identity {}) };
    let mut allocator = PseudoAllocator::new(sys);

    // iterate over the table and initialize the paging system
    for i in 0..table.entries.len() {
        if !table.entries[i].is_present() {
            if i == 0 {
                // there must be at least one entry present
                return Err(ExitCode::InvalidMemoryLayout)
            }
            // break if the last entry is reached
            break;
        }

        // entry is present -> initialize the page
        create_mapping(&mut mapper, &mut allocator, table.entries[i])?;
    }

    Ok(())
}

fn create_mapping<M, A>(
    mapper: &mut MappedPageTable<M>,
    allocator: &mut A,
    entry: LayoutTableEntry,
) -> Result<(), ExitCode>
where
    M: PageTableFrameMapping,
    A: FrameAllocator<Size4KiB> + ?Sized,
{
    let mut addr = entry.addr();
    let end = addr.add(entry.size());
    let mut flags = PageTableFlags::PRESENT;
    flags |= entry.flags().to_page_table_flags();

    while addr < end {
        match addr {
            _ if aligned_and_fits::<Page1GiB>(addr.as_u64(), end.as_u64()) => unsafe {
                flags |= PageTableFlags::HUGE_PAGE;
                let start = x86_64::PhysAddr::new(addr.as_u64());
                let frame: PhysFrame<Size1GiB> = PhysFrame::from_start_address(start).unwrap();

                let flush = mapper.identity_map(frame, flags, allocator)?;
                flush.flush();

                addr += Page1GiB::ALIGNMENT;;
            },
            _ if aligned_and_fits::<Page2MiB>(addr.as_u64(), end.as_u64()) => unsafe {
                flags |= PageTableFlags::HUGE_PAGE;
                let start = x86_64::PhysAddr::new(addr.as_u64());
                let frame: PhysFrame<Size2MiB> = PhysFrame::from_start_address(start).unwrap();

                let flush = mapper.identity_map(frame, flags, allocator)?;
                flush.flush();

                addr += Page2MiB::ALIGNMENT;
            },
            _ => unsafe {
                let start = x86_64::PhysAddr::new(addr.as_u64());
                let frame: PhysFrame<Size2MiB> = PhysFrame::from_start_address(start).unwrap();

                let flush = mapper.identity_map(frame, flags, allocator)?;
                flush.flush();

                addr += Page4KiB::ALIGNMENT;
            },
        }
    }

    Ok(())
}

struct Identity {}

unsafe impl PageTableFrameMapping for Identity {
    fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable {
        let addr = frame.start_address().as_u64();
        unsafe { &mut *(addr as *mut PageTable) }
    }
}

const PAGE_REQ_BY_OTHERS: usize = gdt::PAGE_REQ + idt::PAGE_REQ;

struct PseudoAllocator {
    next: PhysAddr,
    max_allocatable: usize,
    curr_allocated: usize,
}

impl PseudoAllocator {
    pub fn new(entry: LayoutTableEntry) -> Self {
        PseudoAllocator {
            next: entry.addr(),
            max_allocatable: entry.len() as usize - PAGE_REQ_BY_OTHERS,
            curr_allocated: 0,
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for PseudoAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if self.curr_allocated < self.max_allocatable {
            let addr = match x86_64::PhysAddr::try_new(self.next.as_u64()) {
                Ok(addr) => addr,
                Err(_) => return None,
            };

            self.curr_allocated += 1;
            self.next += Size4KiB::SIZE;

            return match PhysFrame::from_start_address(addr) {
                Ok(frame) => Some(frame),
                Err(_) => None,
            };
        }

        None
    }
}

impl<S: PageSize> FrameDeallocator<S> for PseudoAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<S>) {
        // Noop, as currently not intended to unmap and therefore dealloc
    }
}
