use crate::alloc::{Allocator, ReadWrite, Region, RegionCollection};
use crate::elf::ExecBundle;
use crate::linker::{hypercall, upcall};
use crate::vm::registry::{Hypercalls, Upcalls};
use crate::vm::setup::{GDT_PAGE_REQUIRED, GDT_SIZE, IDT_PAGE_REQUIRED, IDT_SIZE};
use crate::vm::vcpu::Vcpu;
use crate::vm::{Config, paging, registry, setup, vcpu};
use crate::{GUEST_PAGING_ADDR, GUEST_STACK_ADDR, GUEST_SYSTEM_ADDR};
use bmvm_common::error::ExitCode;
use bmvm_common::interprete::Interpret;
use bmvm_common::mem;
use bmvm_common::mem::{
    Align, AlignedNonZeroU64, AlignedNonZeroUsize, DefaultAddrSpace, DefaultAlign, Flags,
    LayoutTable, LayoutTableEntry, Page1GiB, Page2MiB, Page4KiB, PhysAddr, Stack, VirtAddr,
    align_floor, init as init_vmi_alloc,
};
use bmvm_common::registry::Params;
use bmvm_common::vmi::{ForeignShareable, Transport};
use bmvm_common::{BMVM_MEM_LAYOUT_TABLE, HYPERCALL_IO_PORT};
use kvm_bindings::KVM_API_VERSION;
use kvm_ioctls::{Cap, Kvm, VcpuExit, VmFd};
use std::io::Write;
use std::num::NonZeroUsize;

const INITIAL_PAGE_ALLOC: usize = 16;
const ADDITIONAL_PAGE_ALLOC: usize = 4;

const SYS_REGION_OFFSET_GDT: u64 = 0;
const SYS_REGION_OFFSET_IDT: u64 = SYS_REGION_OFFSET_GDT + GDT_SIZE;

type Result<T> = core::result::Result<T, Error>;

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
    #[error("Error during paging setup: {0}")]
    Paging(#[from] paging::Error),
    #[error("Memory mapping not found: {0:?}")]
    VmMemoryMappingNotFound(PhysAddr),
    #[error("Memory mapping is not readable: {0:?}")]
    VmMemoryMappingNotReadable(PhysAddr),
    #[error("Memory request exceeds max memory: {0}")]
    VmMemoryRequestExceedsMaxMemory(u64),
    #[error("Error during hypercall execution: {0}")]
    HypercallError(registry::Error),
    #[error("Error during upcall execution: {0}")]
    UpcallInitError(registry::Error),
    #[error("Error during upcall preparation: {0}")]
    UpcallExecError(mem::Error),
    #[error("Error during upcall return: {0}")]
    UpcallReturnError(ExitCode),
    #[error("Guest unexpectedly return with upcall state, without previous upcall call")]
    UnexpectedUpcallReturn,
    #[error("VCPU error: {0}")]
    Vcpu(#[from] vcpu::Error),
    #[error("Setup error: {0}")]
    Setup(#[from] setup::Error),
    #[error("Allocator error: {0}")]
    Allocator(#[from] crate::alloc::Error),
    #[error("Unexpected exit reason: See logs for details")]
    UnexpectedExit,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
enum State {
    PreSetup,
    Ready,
    Executing,
    UpcallExec,
    HypercallExec,
    Shutdown,
}

pub struct Vm {
    cfg: Config,
    state: State,
    kvm: Kvm,
    vm: VmFd,
    vcpu: Vcpu,
    manager: Allocator,
    hypercalls: Hypercalls,
    upcalls: Upcalls,
    mem_mappings: RegionCollection,

    paging_size: usize,
}

impl Vm {
    /// create a new VM instance
    pub(crate) fn new<CONFIG: Into<Config>>(cfg: CONFIG) -> Result<Self> {
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
        let manager = Allocator::new(&vm);

        Ok(Self {
            cfg: cfg.into(),
            state: State::PreSetup,
            kvm,
            vm,
            vcpu,
            manager,
            hypercalls: Hypercalls::default(),
            upcalls: Upcalls::default(),
            mem_mappings: RegionCollection::new(),
            paging_size: 0,
        })
    }

    /// load the guest executable
    pub(crate) fn load_exec(&mut self, exec: &mut ExecBundle) -> Result<()> {
        // allocate a stack region
        let (stack, stack_entry) = self.alloc_stack(self.cfg.stack_size, GUEST_STACK_ADDR())?;
        let stack_addr = stack.addr();
        self.mem_mappings.push(stack);
        exec.layout.push(stack_entry);

        // allocate shared memory managed by the guest and host
        // Memory layout: sys | stack | shared_guest | shared_host | ... | code
        let (shared_guest, shared_guest_entry) = self.alloc_shared_guest(stack_addr)?;
        let (shared_host, shared_host_entry) = self.alloc_shared_host(shared_guest.addr())?;
        // initialize the respective allocators
        let owned = shared_host.as_arena();
        let foreign = shared_guest.as_arena();
        init_vmi_alloc(owned, foreign);
        // include in the execution bundle
        self.mem_mappings.push(shared_guest);
        self.mem_mappings.push(shared_host);
        exec.layout.push(shared_guest_entry);
        exec.layout.push(shared_host_entry);

        // prepare the system region
        let (gdt, idt, paging) = self.setup_long_mode_env(exec)?;

        // move all execution relevant regions to the vm
        self.mem_mappings.append(&mut exec.mem_regions);

        // setup the vcpu for execution
        self.setup_cpu(exec.entry.as_virt_addr(), gdt, idt, paging)?;

        // map all regions to the guest
        for (slot, r) in self.mem_mappings.iter_mut().enumerate() {
            r.set_as_guest_memory(&self.vm, slot as u32)?
        }

        if self.cfg.debug {
            self.vcpu.enable_single_step()?;
        }

        Ok(())
    }

    /// Pass the Host provided VMI function to the VM structure
    pub(crate) fn link(
        &mut self,
        hypercalls: Vec<hypercall::Function>,
        upcalls: Vec<upcall::Function>,
    ) {
        self.hypercalls = Hypercalls::from(hypercalls);
        self.upcalls = Upcalls::from(upcalls);
    }

    /// Expose the guest memory allocator used by this VM instance
    pub(crate) fn allocator(&self) -> &Allocator {
        &self.manager
    }
}

// Implementation regarding the vm execution state
impl Vm {
    /// run the guest
    pub(crate) fn run<R>(&mut self) -> Result<()>
    where
        R: ForeignShareable,
    {
        log::debug!("executing vm");
        loop {
            match self.vcpu.run()? {
                // IO Out should only be triggered by the hypercall
                // execute hypercall or log warning otherwise
                VcpuExit::IoOut(port, data) => {
                    if port == HYPERCALL_IO_PORT {
                        self.hypercall_exec()?;
                    } else {
                        log::warn!(
                            "Unexpected IO write on port {:#x} with data {:X?}",
                            port,
                            data,
                        );
                    }
                }
                // Check the exit code and react accordingly
                VcpuExit::Hlt => {
                    let exit_code = ExitCode::from((self.vcpu.read_regs()?.rax & 0xFF) as u8);
                    match exit_code {
                        ExitCode::Normal => {
                            log::info!("normal exit, shutting down VM");
                            self.state = State::Shutdown;
                        }
                        ExitCode::Ready => {
                            log::info!("guest setup finished");
                            self.state = State::Ready;
                        }
                        ExitCode::Return => {
                            if self.state != State::UpcallExec {
                                return Err(Error::UnexpectedUpcallReturn);
                            }

                            log::info!("guest returned from upcall");
                            self.state = State::UpcallExec;
                        }
                        _ => log::error!("Exit Code: {:?}", exit_code),
                    }
                    self.react_to_exit_code(exit_code)?;

                    return Ok(());
                }
                VcpuExit::Debug(_debug) => {
                    let rip = self.vcpu.read_regs()?.rip;
                    log::debug!("Debug called at RIP: {:#x}", rip);
                    self.print_debug_info()?;
                    self.vcpu.enable_single_step().map_err(Error::Vcpu)?
                }
                // Unexpected Exit
                reason => {
                    log::error!("Unexpected exit reason: {:?}", reason);
                    let _ = &self.print_debug_info()?;
                    let _ = &self.dump_region(0x1000)?;
                    return Err(Error::UnexpectedExit);
                }
            }
        }
    }
}

// Implementation regarding the guest-host interaction
impl Vm {
    /// Setup the guest environment to execute the upcall
    pub fn upcall_exec_setup<P, R>(&mut self, name: &'static str, params: P) -> Result<()>
    where
        P: Params,
        R: ForeignShareable,
    {
        let func = self
            .upcalls
            .find_upcall::<P, R>(name)
            .map_err(Error::UpcallInitError)?;

        let transport = params.into_transport().map_err(Error::UpcallExecError)?;

        self.vcpu.mutate_regs(|regs| {
            // Set the parameters
            regs.r8 = transport.primary();
            regs.r9 = transport.secondary();

            // Set the function pointer
            regs.rip = func.ptr().unwrap().as_u64();
            log::info!("Calling function '{name}' at {:#x}", regs.rip);
            true
        })?;

        self.state = State::UpcallExec;
        Ok(())
    }

    /// Try reading the return value form the previously executed Upcall
    pub fn upcall_result<R>(&mut self) -> Result<R>
    where
        R: ForeignShareable,
    {
        let regs = self.vcpu.read_regs()?;
        let transport = Transport::new(regs.r8, regs.r9);
        R::from_transport(transport).map_err(Error::UpcallReturnError)
    }

    fn hypercall_exec(&mut self) -> Result<()> {
        log::debug!("HYPERCALL TRIGGER");

        // save the current state and set the hypercall state
        let prev = self.state;
        self.state = State::HypercallExec;

        // read hypercall parameters
        let mut regs = self.vcpu.get_regs()?;
        let sig = regs.rbx;
        let transport = Transport::new(regs.r8, regs.r9);
        log::debug!("Parameter: signature={}, transport={}", sig, transport);

        // execute the hypercall
        let output = self
            .hypercalls
            .try_execute(sig, transport)
            .map_err(Error::HypercallError)?;

        // write the result to the registers
        regs.r8 = output.primary();
        regs.r9 = output.secondary();
        log::debug!("Result: transport={}", output);
        self.vcpu.set_regs(regs);

        // restore the previous state
        self.state = prev;
        Ok(())
    }
}

// Implementation regarding initial setup
impl Vm {
    fn align_by_ref(value: u64, reference: u64) -> AlignedNonZeroU64 {
        if Page1GiB::is_aligned(reference) {
            AlignedNonZeroU64::new_aligned(Page1GiB::align_floor(value)).unwrap()
        } else if Page2MiB::is_aligned(reference) {
            AlignedNonZeroU64::new_aligned(Page2MiB::align_floor(value)).unwrap()
        } else {
            AlignedNonZeroU64::new_aligned(Page4KiB::align_floor(value)).unwrap()
        }
    }

    /// allocate memory for the stack
    fn alloc_stack(
        &mut self,
        capacity: AlignedNonZeroUsize,
        base: PhysAddr,
    ) -> Result<(Region<ReadWrite>, LayoutTableEntry)> {
        let region = self
            .manager
            .alloc_accessible::<ReadWrite>(capacity)
            .map_err(|e| Error::Allocator(e))?;

        // stack grows downwards -> mount address is at the top of the stack
        let guest_addr = align_floor((base - capacity.get() as u64).as_u64());
        let phys_addr = PhysAddr::new(guest_addr);
        let stack = region.set_guest_addr(phys_addr);

        let size = (capacity.get() as u64 / DefaultAlign::ALIGNMENT) as u32;
        let entry = LayoutTableEntry::new(
            phys_addr,
            phys_addr.as_virt_addr(),
            size,
            Flags::PRESENT | Flags::DATA_WRITE | Flags::STACK,
        );

        Ok((stack, entry))
    }

    /// allocate shared memory managed by the guest
    fn alloc_shared_guest(
        &mut self,
        upper: PhysAddr,
    ) -> Result<(Region<ReadWrite>, LayoutTableEntry)> {
        let capacity = self.cfg.shared_guest;
        let proto = self.manager.alloc_accessible::<ReadWrite>(capacity)?;

        // ensure same address alignment as the shared memory region
        let addr_base = Self::align_by_ref(
            upper.as_usize() as u64 - capacity.get() as u64,
            proto.as_ptr() as u64,
        );

        // set the address of the region to the aligned address
        let addr = PhysAddr::new(addr_base.get());
        let region = proto.set_guest_addr(addr);

        // construct the layout table entry
        let host_vaddr = region.as_ptr() as u64;
        let size = (self.cfg.shared_guest.get() as u64 / DefaultAlign::ALIGNMENT) as u32;
        let layout = LayoutTableEntry::empty()
            .set_paddr(addr)
            .set_vaddr(VirtAddr::new_truncate(host_vaddr))
            .set_len(size)
            .set_flags(Flags::PRESENT | Flags::DATA_SHARED_OWNED);

        Ok((region, layout))
    }

    /// allocate shared memory managed by the host
    fn alloc_shared_host(
        &mut self,
        upper: PhysAddr,
    ) -> Result<(Region<ReadWrite>, LayoutTableEntry)> {
        let capacity = self.cfg.shared_host;
        let proto = self.manager.alloc_accessible::<ReadWrite>(capacity)?;

        // ensure the same address alignment as the shared memory region
        let addr_base = Self::align_by_ref(
            upper.as_usize() as u64 - capacity.get() as u64,
            proto.as_ptr() as u64,
        );

        // set the address of the region to the aligned address
        let addr = PhysAddr::new(addr_base.get());
        let region = proto.set_guest_addr(addr);

        // construct the layout table entry
        let host_vaddr = region.as_ptr() as u64;
        let size = (self.cfg.shared_guest.get() as u64 / DefaultAlign::ALIGNMENT) as u32;
        let layout = LayoutTableEntry::empty()
            .set_paddr(addr)
            .set_vaddr(VirtAddr::new_truncate(host_vaddr))
            .set_len(size)
            .set_flags(Flags::PRESENT | Flags::DATA_SHARED_FOREIGN);

        Ok((region, layout))
    }

    // TODO: Move to GuestOnly regions (if possible, wait for kernel upgrade)
    /// Setting up a minimal environment containing paging structure, IDT and GDT to be able to enter
    /// long mode and start with the actual structure setup by the guest.
    fn setup_long_mode_env(
        &mut self,
        exec: &mut ExecBundle,
    ) -> Result<(PhysAddr, PhysAddr, PhysAddr)> {
        // allocate a region for the system structures
        let size_sys = AlignedNonZeroUsize::new_ceil((IDT_SIZE + GDT_SIZE) as usize).unwrap();
        let mut sys_region = self
            .manager
            .alloc_accessible::<ReadWrite>(size_sys)?
            .set_guest_addr(GUEST_SYSTEM_ADDR());

        // write GDT
        sys_region.write_offset(SYS_REGION_OFFSET_GDT as usize, setup::gdt().as_ref())?;
        // write LDT
        sys_region.write_offset(SYS_REGION_OFFSET_IDT as usize, setup::idt().as_ref())?;
        self.mem_mappings.push(sys_region);
        exec.layout.push(
            LayoutTableEntry::empty()
                .set_paddr(GUEST_SYSTEM_ADDR())
                .set_vaddr(GUEST_SYSTEM_ADDR().as_virt_addr())
                .set_len((IDT_PAGE_REQUIRED + GDT_PAGE_REQUIRED) as u32)
                .set_flags(Flags::PRESENT | Flags::DATA_WRITE),
        );

        // Empty init the layout region
        let layout = AlignedNonZeroUsize::new_aligned(Page4KiB::ALIGNMENT as usize).unwrap();
        let mut layout_region = self
            .manager
            .alloc_accessible::<ReadWrite>(layout)?
            .set_guest_addr(BMVM_MEM_LAYOUT_TABLE);
        exec.layout.push(
            LayoutTableEntry::empty()
                .set_paddr(BMVM_MEM_LAYOUT_TABLE)
                .set_vaddr(BMVM_MEM_LAYOUT_TABLE.as_virt_addr())
                .set_len(1)
                .set_flags(Flags::PRESENT | Flags::DATA_READ),
        );

        // setup the paging structure
        let regions = paging::setup(
            &self.manager,
            exec.layout.as_slice(),
            GUEST_PAGING_ADDR(),
            NonZeroUsize::new(INITIAL_PAGE_ALLOC).unwrap(),
            NonZeroUsize::new(ADDITIONAL_PAGE_ALLOC).unwrap(),
        )?;

        // fill the layout table with the allocated regions
        let table = LayoutTable::from_mut_bytes(layout_region.as_mut()).unwrap();
        for (i, e) in exec.layout.iter().enumerate() {
            table.entries[i] = e.clone();
        }
        self.mem_mappings.push(layout_region);

        let mut paging_size = 0;
        for r in regions {
            paging_size += r.capacity().get();
            self.mem_mappings.push(r);
        }
        self.paging_size = paging_size;

        let gdt = GUEST_SYSTEM_ADDR() + SYS_REGION_OFFSET_GDT;
        let idt = GUEST_SYSTEM_ADDR() + SYS_REGION_OFFSET_IDT;
        let paging = GUEST_PAGING_ADDR();

        Ok((gdt, idt, paging))
    }

    /// Setting up the Vcpu with pointers to all necessary structures (Paging, IDT, GDT, etc)
    fn setup_cpu(
        &mut self,
        entry_point: VirtAddr,
        gdt: PhysAddr,
        idt: PhysAddr,
        paging: PhysAddr,
    ) -> Result<()> {
        let setup = vcpu::Setup {
            gdt: vcpu::Gdt {
                addr: gdt,
                entries: 3,
                code: 1,
                data: 2,
            },
            idt: vcpu::Idt {
                addr: idt,
                entries: 0,
            },
            paging: paging,
            stack: (GUEST_STACK_ADDR().as_virt_addr() - 1).align_floor::<Stack>(),
            entry: entry_point,
            cpu_id: setup::cpuid(&self.kvm)?,
        };

        self.vcpu.setup(&setup).map_err(|e| Error::Vcpu(e))
    }
}

// Implementation regarding vm debugging
impl Vm {
    /// dump specific region based on exit code
    fn react_to_exit_code(&mut self, code: ExitCode) -> Result<()> {
        match code {
            ExitCode::InvalidMemoryLayoutTableTooSmall => self.dump_region(0x0),
            ExitCode::InvalidMemoryLayoutTableMisaligned => self.dump_region(0x0),
            ExitCode::InvalidMemoryLayout => self.dump_region(0x0),
            _ => Ok(()),
        }
    }

    /// print the basic debug information: registers and optionally the page fault region
    fn print_debug_info(&mut self) -> Result<()> {
        let (regs, sregs) = self.vcpu.read_all_regs()?;
        log::debug!("RIP -> {:#x}", regs.rip);
        // Store the relevant register values to avoid holding the mutable borrow
        let cr2 = sregs.cr2;

        // Print registers before getting memory to avoid borrow conflict
        log::info!("registers -> {:?}", regs);
        log::debug!("Paging Ptr -> {:?}", sregs.cr3);

        if cr2 != 0 {
            log::info!("PAGE FAULT at: cr2 -> {:#x}", cr2);
            self.dump_paging()?;
        }

        Ok(())
    }

    /// dump the region containing the address to console
    fn dump_region(&self, addr: u64) -> Result<()> {
        self.dump_region_to_file(addr, format!("dump_{:#x}.bin", addr))
    }

    fn dump_paging(&self) -> Result<()> {
        let mut file =
            std::fs::File::create(format!("dump_layout_{:x}.bin", BMVM_MEM_LAYOUT_TABLE)).unwrap();
        self.mem_mappings.dump(
            BMVM_MEM_LAYOUT_TABLE,
            Page4KiB::ALIGNMENT as usize,
            &mut file,
        )?;

        let mut file =
            std::fs::File::create(format!("dump_paging_{:x}.bin", GUEST_PAGING_ADDR())).unwrap();
        self.mem_mappings
            .dump(GUEST_PAGING_ADDR(), self.paging_size, &mut file)?;
        Ok(())
    }

    /// dump the region containing the address to file
    fn dump_region_to_file(&self, addr: u64, name: String) -> Result<()> {
        let paddr = PhysAddr::<DefaultAddrSpace>::from(unsafe { VirtAddr::new_unchecked(addr) });
        if let Some(r) = self.mem_mappings.get(paddr) {
            match r.as_ref() {
                Some(reference) => {
                    let mut file = std::fs::File::create(name).unwrap();
                    file.write_all(reference).unwrap();
                    Ok(())
                }
                None => Err(Error::VmMemoryMappingNotReadable(paddr)),
            }
        } else {
            Err(Error::VmMemoryMappingNotFound(paddr))
        }
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        for entry in self.mem_mappings.iter_mut() {
            match entry.remove_from_guest_memory(&self.vm) {
                Ok(_) => (),
                Err(err) => log::warn!("Failed to remove from guest memory: {}", err),
            }
        }
    }
}
