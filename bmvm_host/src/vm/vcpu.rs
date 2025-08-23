use crate::utils::Dirty;
use crate::vm::setup::{GDT_BASE, GDT_ENTRY_SIZE, GDT_LIMIT, IDT_ENTRY_SIZE};
use bmvm_common::mem::{PhysAddr, VirtAddr};
use kvm_bindings::{
    __u16, CpuId, KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, kvm_dtable, kvm_guest_debug,
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
    #[error("Failed to set cpu id: {0}")]
    SetCpuID(kvm_ioctls::Error),
    #[error("Error during execution: {0}")]
    Run(kvm_ioctls::Error),
}

type Result<T> = core::result::Result<T, Error>;

/// CR0: Protection Enabled
const CR0_PE: u64 = 1 << 0;
/// CRO: Extention Type
const CR0_ET: u64 = 1 << 4;
/// CR0: Write Protect
const CR0_WP: u64 = 1 << 16;
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
const EFER_NX: u64 = 0x1 << 11;

pub struct Gdt {
    pub addr: PhysAddr,
    pub entries: usize,
    pub code: u16,
    pub data: u16,
}

pub struct Idt {
    pub addr: PhysAddr,
    pub entries: usize,
}

pub struct Setup {
    pub gdt: Gdt,
    pub idt: Idt,
    pub paging: PhysAddr,
    pub stack: VirtAddr,
    pub entry: VirtAddr,
    pub cpu_id: CpuId,
}

pub struct Vcpu {
    inner: VcpuFd,
    regs: Dirty<kvm_regs>,
    sregs: Dirty<kvm_sregs>,

    recent_exec: bool,
}

// -------------------------------------------------------------------------------------------------
// General
// -------------------------------------------------------------------------------------------------
impl Vcpu {
    pub(crate) fn new(vm: &VmFd, id: u64) -> Result<Self> {
        let inner = vm.create_vcpu(id).map_err(Error::CreateVcpu)?;
        let regs = inner.get_regs().map_err(Error::GetRegs)?;
        let sregs = inner.get_sregs().map_err(Error::GetSregs)?;
        Ok(Self {
            inner,
            regs: Dirty::new(regs),
            sregs: Dirty::new(sregs),
            recent_exec: false,
        })
    }

    pub fn mutate_regs<M>(&mut self, m: M) -> Result<()>
    where
        M: FnOnce(&mut kvm_regs) -> bool,
    {
        self.refresh_regs()?;
        self.regs.mutate(m);
        Ok(())
    }

    pub fn set_regs(&mut self, regs: kvm_regs) {
        self.regs.set(regs)
    }

    pub fn get_regs(&mut self) -> Result<kvm_regs> {
        self.refresh_regs()?;
        Ok(*self.regs.get())
    }

    pub fn read_regs(&mut self) -> Result<&kvm_regs> {
        self.refresh_regs()?;
        Ok(self.regs.get())
    }

    pub fn read_all_regs(&mut self) -> Result<(&kvm_regs, &kvm_sregs)> {
        self.refresh_regs()?;
        Ok((self.regs.get(), self.sregs.get()))
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
        let regs = self.inner.get_regs().map_err(Error::GetRegs)?;
        self.regs.set(regs);
        Ok(())
    }

    #[inline]
    fn fetch_sregs(&mut self) -> Result<()> {
        let sregs = self.inner.get_sregs().map_err(Error::GetSregs)?;
        self.sregs.set(sregs);
        Ok(())
    }

    #[inline]
    fn propagate_regs(&mut self) -> Result<()> {
        self.regs
            .sync(|regs| self.inner.set_regs(regs).map_err(Error::SetRegs))
            .unwrap_or(Result::<()>::Ok(()))
    }

    #[inline]
    fn propagate_sregs(&mut self) -> Result<()> {
        self.sregs
            .sync(|sregs| self.inner.set_sregs(sregs).map_err(Error::SetSregs))
            .unwrap_or(Result::<()>::Ok(()))
    }
}

// -------------------------------------------------------------------------------------------------
// Setup
// -------------------------------------------------------------------------------------------------
impl Vcpu {
    /// set up all required pointer and control registers for execution
    pub fn setup(&mut self, setup: &Setup) -> Result<()> {
        self.setup_cpuid(&setup.cpu_id)?;
        self.setup_gdt(&setup.gdt)?;
        self.setup_idt(&setup.idt)?;
        self.setup_paging(setup.paging)?;
        self.setup_execution(setup.stack, setup.entry)?;
        Ok(())
    }

    /// set up the CPUID functions supported by the vcpu in guest mode
    fn setup_cpuid(&mut self, cpu_id: &CpuId) -> Result<()> {
        self.inner.set_cpuid2(cpu_id).map_err(Error::SetCpuID)
    }

    /// set up the Global Descriptor Table pointer and related segments
    fn setup_gdt(&mut self, gdt: &Gdt) -> Result<()> {
        self.refresh_regs()?;

        self.sregs.mutate(|sregs| {
            sregs.gdt = kvm_dtable {
                base: gdt.addr.as_u64(),
                limit: (gdt.entries * GDT_ENTRY_SIZE) as __u16 - 1,
                padding: [0; 3usize],
            };

            sregs.cs.selector = gdt.code * GDT_ENTRY_SIZE as u16;
            sregs.cs.base = GDT_BASE;
            sregs.cs.limit = GDT_LIMIT as u32;
            sregs.cs.present = 1;
            sregs.cs.type_ = 0xB;
            sregs.cs.s = 1;
            sregs.cs.dpl = 0;
            sregs.cs.db = 0;
            sregs.cs.l = 1;
            sregs.cs.g = 1;

            sregs.ss.selector = gdt.data * GDT_ENTRY_SIZE as u16;
            sregs.ss.base = 0;
            sregs.ss.limit = GDT_LIMIT as u32;
            sregs.ss.present = 1;
            sregs.ss.type_ = 0x2;
            sregs.ss.s = 1;
            sregs.ss.dpl = 0;
            sregs.ss.l = 0;
            sregs.ss.g = 1;

            sregs.ds.selector = gdt.data * GDT_ENTRY_SIZE as u16;
            sregs.ds.base = 0;
            sregs.ds.limit = GDT_LIMIT as u32;
            sregs.ds.present = 1;
            sregs.ds.type_ = 0x2;
            sregs.ds.s = 1;
            sregs.ds.dpl = 0;
            sregs.ds.l = 0;
            sregs.ds.g = 1;

            sregs.es.selector = gdt.data * GDT_ENTRY_SIZE as u16;
            sregs.es.base = 0;
            sregs.es.limit = GDT_LIMIT as u32;
            sregs.es.present = 1;
            sregs.es.type_ = 0x2;
            sregs.es.s = 1;
            sregs.es.dpl = 0;
            sregs.es.l = 0;
            sregs.es.g = 1;

            true
        });

        Ok(())
    }

    /// set up the Interrupt Descriptor Table pointer
    fn setup_idt(&mut self, idt: &Idt) -> Result<()> {
        self.refresh_regs()?;

        self.sregs.mutate(|sregs| {
            sregs.idt = kvm_dtable {
                base: idt.addr.as_u64(),
                limit: (idt.entries * IDT_ENTRY_SIZE) as __u16,
                padding: [0; 3usize],
            };
            true
        });

        Ok(())
    }

    /// set up the control registers for long mode with paging
    fn setup_paging(&mut self, addr: PhysAddr) -> Result<()> {
        self.refresh_regs()?;

        self.sregs.mutate(|sregs| {
            // enable protected mode and paging
            sregs.cr0 = CR0_PE | CR0_PG | CR0_ET | CR0_WP;
            // set the paging address
            sregs.cr3 = addr.as_u64();
            // set Debug, and Physical-Address Extension, Page-Global Enable
            sregs.cr4 = CR4_DE | CR4_PSE | CR4_PAE | CR4_PGE;
            // set Long-Mode Active and Long-Mode Enabled
            sregs.efer |= EFER_LMA | EFER_LME | EFER_NX;
            true
        });

        Ok(())
    }

    /// set up other execution relevant registers besides the structures required for long mode
    fn setup_execution(&mut self, stack: VirtAddr, entry: VirtAddr) -> Result<()> {
        log::debug!(
            "Setting up execution - Stack: {:x} ({}) Entry: {:x}",
            stack,
            stack.as_u64(),
            entry
        );

        self.refresh_regs()?;

        self.regs.mutate(|regs| {
            regs.rflags = 1 << 1;
            regs.rip = entry.as_u64();
            regs.rsp = stack.as_u64();
            true
        });

        Ok(())
    }
}

// -------------------------------------------------------------------------------------------------
// Execution
// -------------------------------------------------------------------------------------------------
impl Vcpu {
    /// Enable single stepping for the next instruction. By enabling this feature, the guest will
    /// exit with `VcpuExit::Debug` after executing the next instruction
    pub fn enable_single_step(&mut self) -> Result<()> {
        let dbg = kvm_guest_debug {
            control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
            pad: 0,
            arch: kvm_guest_debug_arch { debugreg: [0; 8] },
        };
        self.inner
            .set_guest_debug(&dbg)
            .map_err(Error::SetGuestDebug)?;

        self.regs.mutate(|regs| {
            regs.rflags |= 1 << 8;
            true
        });

        Ok(())
    }

    /// Run the Vcpu by propagating any register changes made by the host to the guest and execute.
    pub fn run(&mut self) -> Result<VcpuExit<'_>> {
        self.propagate_regs()?;
        self.propagate_sregs()?;

        let exit = self.inner.run().map_err(Error::Run);
        self.recent_exec = true;
        exit
    }
}
