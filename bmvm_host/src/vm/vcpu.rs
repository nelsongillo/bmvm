use crate::utils::Dirty;
use bitflags::bitflags;
use kvm_bindings::{
    KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, kvm_guest_debug, kvm_guest_debug_arch, kvm_regs,
    kvm_sregs,
};
use kvm_ioctls::{VcpuExit, VcpuFd, VmFd};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create vcpu: {0}")]
    CreateVcpu(kvm_ioctls::Error),
    #[error("Failed to set regs: {0}")]
    SetRegs(kvm_ioctls::Error),
    #[error("Failed to set sregs: {0}")]
    SetSregs(kvm_ioctls::Error),
    #[error("Failed to get regs: {0}")]
    GetRegs(kvm_ioctls::Error),
    #[error("Failed to get sregs: {0}")]
    GetSregs(kvm_ioctls::Error),
    #[error("Failed to set guest debug: {0}")]
    SetGuestDebug(kvm_ioctls::Error),
    #[error("Error during execution: {0}")]
    Run(kvm_ioctls::Error),
}

type Result<T> = core::result::Result<T, Error>;

bitflags! {
    pub struct CR4Flags: u64 {
        const DE        = 0x1 <<  3;
        const OSFXSR    = 0x1 <<  9;
        const OSXSAVE   = 0x1 << 18;
    }
}

bitflags! {
    pub struct EFERFlags: u64 {
        const LME        = 0x1 <<  8;
        const LMA        = 0x1 << 10;
    }
}

pub struct Vcpu {
    inner: VcpuFd,
    regs: Dirty<kvm_regs>,
    sregs: Dirty<kvm_sregs>,

    recent_exec: bool,
}

impl Vcpu {
    pub(crate) fn new(vm: &VmFd, id: u64) -> Result<Self> {
        let inner = vm.create_vcpu(id).map_err(|e| Error::CreateVcpu(e))?;
        let regs = inner.get_regs().map_err(|e| Error::GetRegs(e))?;
        let sregs = inner.get_sregs().map_err(|e| Error::GetSregs(e))?;
        Ok(Self {
            inner,
            regs: Dirty::new(regs),
            sregs: Dirty::new(sregs),
            recent_exec: false,
        })
    }

    pub fn set_regs(&mut self, regs: kvm_regs) {
        self.regs.set(regs);
    }

    pub fn set_sregs(&mut self, sregs: kvm_sregs) {
        self.sregs.set(sregs);
    }

    pub fn get_regs(&mut self) -> Result<&kvm_regs> {
        self.refresh_regs()?;
        Ok(self.regs.get())
    }

    pub fn get_sregs(&mut self) -> Result<&kvm_sregs> {
        self.refresh_regs()?;
        Ok(self.sregs.get())
    }

    pub fn setup_registers(&mut self) -> Result<()> {
        // Special Register
        self.refresh_regs()?;
        self.sregs.mutate(|sregs| {
            sregs.cr0 = 0x0;
            sregs.cr4 = (CR4Flags::DE | CR4Flags::OSFXSR | CR4Flags::OSXSAVE).bits();
            sregs.efer = (EFERFlags::LMA | EFERFlags::LME).bits();
            true
        });

        // "Normal" Register
        self.regs.mutate(|regs| {
            regs.rflags = 0x2;
            regs.rdx = 0x0;
            true
        });

        Ok(())
    }

    pub fn enable_single_step(&self) -> Result<()> {
        let dbg = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: kvm_guest_debug_arch { debugreg: [0; 8] },
        };
        self.inner
            .set_guest_debug(&dbg)
            .map_err(|e| Error::SetGuestDebug(e))?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<VcpuExit> {
        self.propagate_regs()?;
        self.propagate_sregs()?;

        let exit = self.inner.run().map_err(|e| Error::Run(e));
        self.recent_exec = true;
        exit
    }

    fn refresh_regs(&mut self) -> Result<()> {
        if !self.recent_exec {
            return Ok(());
        }

        self.fetch_regs()?;
        self.fetch_sregs()?;
        self.recent_exec = false;
        Ok(())
    }

    #[inline]
    fn fetch_regs(&mut self) -> Result<()> {
        let regs = self.inner.get_regs().map_err(|e| Error::GetRegs(e))?;
        self.regs.set(regs);
        Ok(())
    }

    #[inline]
    fn fetch_sregs(&mut self) -> Result<()> {
        let sregs = self.inner.get_sregs().map_err(|e| Error::GetSregs(e))?;
        self.sregs.set(sregs);
        Ok(())
    }

    #[inline]
    fn propagate_regs(&mut self) -> Result<()> {
        self.regs
            .sync(|regs| self.inner.set_regs(regs).map_err(|e| Error::SetRegs(e)))
            .unwrap_or(Result::<()>::Ok(()))
    }

    #[inline]
    fn propagate_sregs(&mut self) -> Result<()> {
        self.sregs
            .sync(|sregs| self.inner.set_sregs(sregs).map_err(|e| Error::SetSregs(e)))
            .unwrap_or(Result::<()>::Ok(()))
    }
}
