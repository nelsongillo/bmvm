use crate::alloc::Manager;
use crate::vm::vcpu::Vcpu;
use kvm_bindings::KVM_API_VERSION;
use kvm_ioctls::{Cap, Kvm, VmFd};

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("KVM error: {0:?}")]
    Kvm(kvm_ioctls::Error),
    #[error("KVM API version mismatch: {0}")]
    KvmApiVersionMismatch(i32),
    #[error("KVM missing capability: {0:?}")]
    KvmMissingCapability(Cap),
    #[error("VM error: {0:?}")]
    Vm(kvm_ioctls::Error),
    #[error("VCPU error: {0:?}")]
    Vcpu(kvm_ioctls::Error),
}

struct VM {
    kvm: Kvm,
    vm: VmFd,
    vcpu: Vcpu,
    manager: Manager,
}

impl VM {
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
        let kvm_vcpu = vm.create_vcpu(0).map_err(|e| Error::Vcpu(e))?;
        let mut vcpu = Vcpu::try_from(kvm_vcpu).map_err(|e| Error::Vcpu(e))?;

        // create a region manager
        let manager = Manager::new(&vm);

        Ok(Self {
            kvm,
            vm,
            vcpu,
            manager,
        })
    }
}
