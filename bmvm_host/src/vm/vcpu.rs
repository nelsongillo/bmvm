use bitflags::bitflags;
use kvm_bindings::{
    KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, kvm_guest_debug, kvm_guest_debug_arch, kvm_regs,
    kvm_sregs,
};
use kvm_ioctls::{VcpuExit, VcpuFd};

type Result<T> = std::result::Result<T, kvm_ioctls::Error>;

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
    fd: VcpuFd,
    regs: kvm_regs,
    sregs: kvm_sregs,

    dirty_regs: bool,
    dirty_sregs: bool,
    recently_executed: bool,
}

impl TryFrom<VcpuFd> for Vcpu {
    type Error = kvm_ioctls::Error;

    fn try_from(fd: VcpuFd) -> Result<Self> {
        let regs = fd.get_regs()?;
        let sregs = fd.get_sregs()?;

        Ok(Vcpu {
            fd,
            regs,
            sregs,
            dirty_regs: false,
            dirty_sregs: false,
            recently_executed: false,
        })
    }
}

impl Vcpu {
    pub fn set_regs(&mut self, regs: kvm_regs) {
        self.regs = regs;
        self.dirty_regs = true;
    }

    pub fn set_sregs(&mut self, sregs: kvm_sregs) {
        self.sregs = sregs;
        self.dirty_sregs = true;
    }

    pub fn get_regs(&mut self) -> Result<kvm_regs> {
        self.refresh_regs()?;
        Ok(self.regs.clone())
    }

    pub fn get_sregs(&mut self) -> Result<kvm_sregs> {
        self.refresh_regs()?;
        Ok(self.sregs.clone())
    }

    pub fn setup(&mut self) -> Result<()> {
        // Special Register
        self.refresh_regs()?;
        self.sregs.cr0 = 0x0;
        self.sregs.cr4 = (CR4Flags::DE | CR4Flags::OSFXSR | CR4Flags::OSXSAVE).bits();
        self.sregs.efer = (EFERFlags::LMA | EFERFlags::LME).bits();
        self.dirty_regs = true;

        // "Normal" Register
        self.regs.rflags = 0x2;
        self.regs.rdx = 0x0;
        self.dirty_sregs = true;

        Ok(())
    }

    pub fn enable_single_step(&self) -> Result<()> {
        let dbg = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: kvm_guest_debug_arch { debugreg: [0; 8] },
        };
        self.fd.set_guest_debug(&dbg)?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<VcpuExit> {
        if self.dirty_regs {
            self.fd.set_regs(&self.regs)?;
            self.dirty_regs = false;
        }

        if self.dirty_sregs {
            self.fd.set_sregs(&self.sregs)?;
            self.dirty_sregs = false;
        }

        let exit = self.fd.run();
        self.recently_executed = true;
        exit
    }

    fn refresh_regs(&mut self) -> Result<()> {
        if !self.recently_executed {
            return Ok(());
        }

        self.regs = self.fd.get_regs()?;
        self.sregs = self.fd.get_sregs()?;
        self.recently_executed = false;
        Ok(())
    }
}
