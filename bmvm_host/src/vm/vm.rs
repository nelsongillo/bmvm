use crate::BMVM_GUEST_TMP_SYSTEM_SIZE;
use crate::alloc::{Manager, ReadWrite};
use crate::elf::ExecBundle;
use crate::vm::vcpu;
use crate::vm::vcpu::Vcpu;
use bmvm_common::mem::LayoutTableEntry;
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, BMVM_TMP_GDT, BMVM_TMP_IDT, BMVM_TMP_PAGING};
use kvm_bindings::KVM_API_VERSION;
use kvm_ioctls::{Cap, Kvm, VmFd};
use std::num::NonZeroUsize;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("KVM error: {0:?}")]
    Kvm(kvm_ioctls::Error),
    #[error("KVM API version mismatch: {0}")]
    KvmApiVersionMismatch(i32),
    #[error("KVM missing capability: {0:?}")]
    KvmMissingCapability(Cap),
    #[error("VM error: {0:?}")]
    Vm(kvm_ioctls::Error),
    #[error("VCPU error: {0:?}")]
    Vcpu(#[from] vcpu::Error),
}

pub struct Vm {
    kvm: Kvm,
    vm: VmFd,
    vcpu: Vcpu,
    manager: Manager,
}

impl Vm {
    pub fn new() -> Result<Self, Error> {
        let kvm = Kvm::new().map_err(|e| Error::Kvm(e))?;
        let version = kvm.get_api_version();
        if version != KVM_API_VERSION as i32 {
            return Err(Error::KvmApiVersionMismatch(version));
        }

        // Check KVM_CAP_USER_MEMORY is available (needed
        if !kvm.check_extension(Cap::UserMemory) {
            return Err(Error::KvmMissingCapability(Cap::UserMemory));
        }

        // create a kvm vm instance
        let vm = kvm.create_vm_with_type(0).map_err(|e| Error::Vm(e))?;

        // create a vcpu
        let vcpu = Vcpu::new(&vm, 0)?;

        // create a region manager
        let manager = Manager::new(&vm);

        Ok(Self {
            kvm,
            vm,
            vcpu,
            manager,
        })
    }

    /// Setting up a minimal environment containing paging structure, IDT and GDT to be able to enter
    /// long mode and start with the actual structure setup by the guest.
    fn setup_long_mode_env(&mut self, exec: &ExecBundle, manager: &Manager) -> anyhow::Result<()> {
        // allocate a region for the temporary system strutures
        let size_tmp_sys = NonZeroUsize::new(BMVM_GUEST_TMP_SYSTEM_SIZE as usize).unwrap();
        let mut temp_sys_region = manager.alloc_accessible::<ReadWrite>(size_tmp_sys)?;

        // estimate the system region
        let mut layout = exec.layout.clone();
        let layout_sys = crate::vm::setup::estimate_sys_region(&layout)?;
        layout.insert(0, layout_sys);

        // write GDT
        temp_sys_region.write_offset(BMVM_TMP_GDT.as_usize(), crate::vm::setup::gdt().as_ref())?;
        // write LDT
        temp_sys_region.write_offset(BMVM_TMP_IDT.as_usize(), crate::vm::setup::idt().as_ref())?;
        // write paging
        for (idx, entry) in crate::vm::setup::paging(&layout).iter() {
            let offset = idx * 8;
            temp_sys_region.write_offset(BMVM_TMP_PAGING.as_u64() as usize + offset, entry)?;
        }

        // write layout table
        for (i, entry) in layout.iter().enumerate() {
            let offset = i * size_of::<LayoutTableEntry>();
            temp_sys_region
                .write_offset(BMVM_MEM_LAYOUT_TABLE.as_usize() + offset, &entry.as_array())?;
        }

        Ok(())
    }
}
