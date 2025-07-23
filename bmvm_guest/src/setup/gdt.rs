use crate::setup::GDT_SPACE_REQ;
use bmvm_common::BMVM_TMP_GDT;
use bmvm_common::error::ExitCode;
use bmvm_common::mem::LayoutTableEntry;
use core::ptr;

// Setup Global Descriptor Table
// Here we simply copy the GDT provided by the host to guest memory
pub fn setup(sys: &LayoutTableEntry, offset: u64) -> Result<(), ExitCode> {
    let src = BMVM_TMP_GDT.as_ptr::<u8>();
    let dst = (sys.addr_raw() + offset) as *mut u8;

    unsafe {
        ptr::copy_nonoverlapping(src, dst, GDT_SPACE_REQ as usize);
    }

    Ok(())
}
