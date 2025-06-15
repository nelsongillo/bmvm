use crate::utils::Dirty;
use crate::vm::setup::{GDT_SIZE, IDT_SIZE};
use bmvm_common::mem::VirtAddr;
use kvm_bindings::{
    __u16, KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, kvm_dtable, kvm_guest_debug,
    kvm_guest_debug_arch, kvm_regs, kvm_sregs,
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

/// CR0: Protection Enabled
const CR0_PE: u64 = 1 << 0;
/// CR0: Paging
const CR0_PG: u64 = 1 << 31;

/// CR4: Debugging Extensions
const CR4_DE: u64 = 0x1 << 3;

/// CR4: Page Size Extension
const CR4_PSE: u64 = 0x1 << 4;
/// CR4: Physical-Address Extension
const CR4_PAE: u64 = 0x1 << 5;
/// CR4: Page-Global Enable
const CR4_PGE: u64 = 0x1 << 7;

/// Long Mode Enabled
const EFER_LME: u64 = 0x1 << 8;
/// Long Mode Active
const EFER_LMA: u64 = 0x1 << 10;

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

    pub fn get_all_regs(&mut self) -> Result<(&kvm_regs, &kvm_sregs)> {
        self.refresh_regs()?;
        Ok((self.regs.get(), self.sregs.get()))
    }

    pub fn setup_registers(
        &mut self,
        paging: VirtAddr,
        gdt: VirtAddr,
        idt: VirtAddr,
        stack: VirtAddr,
        entry: VirtAddr,
    ) -> Result<()> {
        // Special Register
        self.refresh_regs()?;
        self.sregs.mutate(|sregs| {
            // set GDT (Guest will use LGDT later to update the GDT localtion)
            sregs.gdt = kvm_dtable {
                base: gdt.as_u64(),
                limit: GDT_SIZE as __u16,
                padding: [0; 3usize],
            };

            // Point to code selector
            sregs.cs.selector = 0x08;
            sregs.cs.base = 0;
            sregs.cs.l = 1;

            // Point to data selector
            sregs.ds.selector = 0x10;
            sregs.ds.base = 0;
            sregs.ds.l = 1;

            // set IDT (Guest will use LIDT later to update the IDT localtion)
            sregs.idt = kvm_dtable {
                base: idt.as_u64(),
                limit: IDT_SIZE as __u16,
                padding: [0; 3usize],
            };

            // enable protected mode and paging
            sregs.cr0 = CR0_PE | CR0_PG;
            // set the paging address
            sregs.cr3 = paging.as_u64();
            // set DEbug, and Physical-Address Extension, Page-Global Enable
            sregs.cr4 = CR4_DE | CR4_PSE | CR4_PAE | CR4_PGE;
            // set Long-Mode Active and Long-Mode Enabled
            sregs.efer |= EFER_LMA | EFER_LME;
            true
        });

        // "Normal" Register
        self.regs.mutate(|regs| {
            regs.rax = u64::MAX;
            regs.rflags = 1 << 1;
            regs.rflags |= 0x100;
            regs.rip = entry.as_u64();
            regs.rsp = stack.as_u64();
            true
        });

        Ok(())
    }

    pub fn enable_single_step(&mut self) -> Result<()> {
        let dbg = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: kvm_guest_debug_arch { debugreg: [0; 8] },
        };
        self.inner
            .set_guest_debug(&dbg)
            .map_err(|e| Error::SetGuestDebug(e))?;

        self.regs.mutate(|regs| {
            regs.rflags |= 0x100;
            true
        });

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
