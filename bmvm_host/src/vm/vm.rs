use crate::alloc::{Allocator, ReadWrite, Region};
use crate::elf::ExecBundle;
use crate::vm::vcpu::Vcpu;
use crate::vm::{setup, vcpu};
use crate::{GUEST_DEFAULT_STACK_SIZE, GUEST_STACK_ADDR, GUEST_TMP_SYSTEM_SIZE};
use bmvm_common::mem::{
    Align, AlignedNonZeroUsize, DefaultAlign, Flags, LayoutTableEntry, Page4KiB, PhysAddr,
    align_floor,
};
use bmvm_common::{
    BMVM_MEM_LAYOUT_TABLE, BMVM_TMP_GDT, BMVM_TMP_IDT, BMVM_TMP_PAGING, BMVM_TMP_SYS,
};
use kvm_bindings::{KVM_API_VERSION, kvm_userspace_memory_region};
use kvm_ioctls::{Cap, Kvm, VcpuExit, VmFd};
use std::io::Write;

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
    #[error("Memory mapping not found: {0:?}")]
    VmMemoryMappingNotFound(PhysAddr),
    #[error("Memory request exceeds max memory: {0}")]
    VmMemoryRequestExceedsMaxMemory(u64),
    #[error("VCPU error: {0:?}")]
    Vcpu(#[from] vcpu::Error),
    #[error("Setup error: {0:?}")]
    Setup(#[from] setup::Error),
    #[error("Allocator error: {0:?}")]
    Allocator(#[from] crate::alloc::Error),
}

#[derive(Debug)]
pub struct Config {
    stack_size: AlignedNonZeroUsize,
    max_memory: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            stack_size: AlignedNonZeroUsize::new_ceil(GUEST_DEFAULT_STACK_SIZE).unwrap(),
            max_memory: 1024 * 1024 * 1024,
        }
    }
}

pub struct Vm {
    cfg: Config,
    kvm: Kvm,
    vm: VmFd,
    vcpu: Vcpu,
    manager: Allocator,
    mem_mappings: Vec<Region<ReadWrite>>,
}

impl Vm {
    pub(crate) fn new<C: Into<Config>>(cfg: C) -> Result<Self> {
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
            kvm,
            vm,
            vcpu,
            manager,
            mem_mappings: Vec::new(),
        })
    }

    pub(crate) fn load_exec(&mut self, exec: &mut ExecBundle) -> Result<()> {
        // allocate a stack region
        let (stack, stack_entry) = self.alloc_stack(self.cfg.stack_size, GUEST_STACK_ADDR())?;
        self.mem_mappings.push(stack);
        exec.layout.push(stack_entry);

        // prepare the system region
        let sys = self.setup_long_mode_env(exec)?;
        self.mem_mappings.push(sys);

        // move all execution relevant regions to the vm
        self.mem_mappings.append(&mut exec.mem_regions);

        // setup the CPU registers
        self.vcpu.setup_registers(
            BMVM_TMP_PAGING.as_virt_addr(),
            BMVM_TMP_GDT.as_virt_addr(),
            BMVM_TMP_IDT.as_virt_addr(),
            GUEST_STACK_ADDR().as_virt_addr(),
            exec.entry.as_virt_addr(),
        )?;

        unsafe {
            // map all regions to the guest
            for (slot, r) in self.mem_mappings.iter().enumerate() {
                let mapping = kvm_userspace_memory_region {
                    slot: slot as u32,
                    flags: 0,
                    guest_phys_addr: r.guest_addr().as_u64(),
                    memory_size: r.capacity() as u64,
                    userspace_addr: r.as_ptr() as u64,
                };
                self.vm
                    .set_user_memory_region(mapping)
                    .map_err(|e| Error::Vm(e))?;
            }
        }

        // #[cfg(feature = "debug")]
        self.vcpu.enable_single_step()?;

        Ok(())
    }

    pub(crate) fn run(&mut self) -> Result<()> {
        loop {
            match self.vcpu.run()? {
                VcpuExit::IoOut(port, data) => {
                    println!(
                        "IO write on port {:#x} with data {:#x} -> {}",
                        port, data[0], data[0] as char
                    )
                }
                VcpuExit::IoIn(port, data) => {
                    println!("IO read on port {:#x} with data {:#x}", port, data[0])
                }
                VcpuExit::Hlt => {
                    println!("Halt called -> shutdown vm");
                    break;
                }
                VcpuExit::Debug(_) => {
                    println!("Debug called");
                    self.print_debug_info()?;
                    println!("\n\n\n");
                }
                reason => {
                    println!("Unexpected exit reason: {:?}", reason);
                    self.print_debug_info()?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Expose the guest memory allocator used by this VM instance
    pub fn allocatort(&self) -> &Allocator {
        &self.manager
    }

    fn alloc_stack(
        &mut self,
        capacity: AlignedNonZeroUsize,
        base: PhysAddr,
    ) -> Result<(Region<ReadWrite>, LayoutTableEntry)> {
        let proto = self
            .manager
            .alloc_accessible::<ReadWrite>(capacity)
            .map_err(|e| Error::Allocator(e))?;

        // stack grows downwards -> mount address is at the top of the stack
        let guest_addr = align_floor((base - capacity.get() as u64).as_u64());
        let region = proto.set_guest_addr(PhysAddr::new(guest_addr));

        let size = (capacity.get() as u64 / DefaultAlign::ALIGNMENT) as u32;
        let entry = LayoutTableEntry::new(base, size, Flags::PRESENT | Flags::WRITE);

        Ok((region, entry))
    }

    /// Setting up a minimal environment containing paging structure, IDT and GDT to be able to enter
    /// long mode and start with the actual structure setup by the guest.
    fn setup_long_mode_env(&mut self, exec: &ExecBundle) -> Result<Region<ReadWrite>> {
        // allocate a region for the temporary system structures
        let size_tmp_sys = AlignedNonZeroUsize::new_ceil(GUEST_TMP_SYSTEM_SIZE as usize).unwrap();
        let mut temp_sys_region = self
            .manager
            .alloc_accessible::<ReadWrite>(size_tmp_sys)?
            .set_guest_addr(BMVM_TMP_SYS);

        // estimate the system region
        let mut layout = exec.layout.clone();
        let layout_sys = setup::estimate_sys_region(&layout)?;
        layout.insert(0, layout_sys);

        // write GDT
        temp_sys_region.write_offset(BMVM_TMP_GDT.as_usize(), setup::gdt().as_ref())?;
        // write LDT
        temp_sys_region.write_offset(BMVM_TMP_IDT.as_usize(), setup::idt().as_ref())?;
        // write paging
        let entries = setup::paging(&layout);
        for (idx, entry) in entries.iter() {
            let write_to = BMVM_TMP_PAGING.as_usize() + idx * 8;
            temp_sys_region.write_offset(write_to, entry)?;
        }

        // write layout table
        for (i, entry) in layout.iter().enumerate() {
            let offset = i * size_of::<LayoutTableEntry>();
            temp_sys_region
                .write_offset(BMVM_MEM_LAYOUT_TABLE.as_usize() + offset, &entry.as_array())?;
        }

        Ok(temp_sys_region)
    }

    fn print_debug_info(&mut self) -> Result<()> {
        let (regs, sregs) = self.vcpu.get_all_regs()?;
        // Store the relevant register values to avoid holding the mutable borrow
        let rip = regs.rip;
        let cr2 = sregs.cr2;
        let rsp = regs.rsp;
        let cr3 = sregs.cr3;

        // Print registers before getting memory to avoid borrow conflict
        println!("DEBUG: registers -> {:?}", regs);
        println!("DEBUG: special registers -> {:?}", sregs);

        if cr2 != 0 {
            println!("PAGE FAULT at: cr2 -> {:#x}", cr2);
            // let mem = self.get_memory_at(align_floor(cr2), Page4KiB::ALIGNMENT)?;
            // println!("DEBUG: memory at {:x} -> {:x?}", cr2, mem.as_slice());

            let stack = self.get_memory_at(align_floor(rsp - 1), Page4KiB::ALIGNMENT)?;
            // println!("DEBUG: memory at {:x} -> {:x?}", rsp, stack.as_slice());
        }

        let mem = self.get_memory_at(rip, 8)?;
        // println!("DEBUG: memory at {:x} -> {:x?}", rip, mem.as_slice());

        self.dump_region_to_file(cr3)?;
        self.dump_region_to_file(rsp)?;

        Ok(())
    }

    /// Try reading the guest memory
    fn get_memory_at(&self, addr: u64, length: u64) -> Result<Vec<u8>> {
        let end = addr + length - 1;
        for r in self.mem_mappings.iter() {
            let guest_addr = r.guest_addr().as_u64();
            let size = r.capacity();
            if (guest_addr..(guest_addr + size as u64)).contains(&addr) {
                if end >= guest_addr + size as u64 {
                    return Err(Error::VmMemoryRequestExceedsMaxMemory(length));
                }

                let v_start = addr - guest_addr;
                let v_end = end - guest_addr;
                return Ok(r.as_ref()[v_start as usize..v_end as usize].to_vec());
            }
        }

        Err(Error::VmMemoryMappingNotFound(PhysAddr::new(addr)))
    }

    fn dump_region_to_file(&self, addr: u64) -> Result<()> {
        for r in self.mem_mappings.iter() {
            let guest_addr = r.guest_addr().as_u64();
            let size = r.capacity();
            if (guest_addr..(guest_addr + size as u64)).contains(&addr) {
                let mut file = std::fs::File::create(format!("dump_{:#x}.bin", addr)).unwrap();
                file.write_all(r.as_ref()).unwrap();
                return Ok(());
            }
        }

        Err(Error::VmMemoryMappingNotFound(PhysAddr::new(addr)))
    }
}
