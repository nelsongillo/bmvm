use crate::vm;
use crate::vm::Vm;
use bmvm_common::mem::{PhysAddr, VirtAddr};
use gdbstub::arch::Arch;
use gdbstub::target::ext::auxv::AuxvOps;
use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::single_register_access::{
    SingleRegisterAccess, SingleRegisterAccessOps,
};
use gdbstub::target::ext::base::singlethread::{SingleThreadBase, SingleThreadResumeOps};
use gdbstub::target::ext::breakpoints::BreakpointsOps;
use gdbstub::target::ext::catch_syscalls::CatchSyscallsOps;
use gdbstub::target::ext::exec_file::ExecFileOps;
use gdbstub::target::ext::extended_mode::ExtendedModeOps;
use gdbstub::target::ext::flash::FlashOps;
use gdbstub::target::ext::host_io::HostIoOps;
use gdbstub::target::ext::libraries::LibrariesSvr4Ops;
use gdbstub::target::ext::lldb_register_info_override::LldbRegisterInfoOverrideOps;
use gdbstub::target::ext::memory_map::MemoryMapOps;
use gdbstub::target::ext::monitor_cmd::MonitorCmdOps;
use gdbstub::target::ext::section_offsets::SectionOffsetsOps;
use gdbstub::target::ext::target_description_xml_override::TargetDescriptionXmlOverrideOps;
use gdbstub::target::ext::tracepoints::TracepointsOps;
use gdbstub::target::{Target, TargetError, TargetResult};
use gdbstub_arch::x86::reg::id::X86_64CoreRegId;
use gdbstub_arch::x86::reg::{X86_64CoreRegs, X86SegmentRegs};
use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("VM error: {0}")]
    Vm(#[from] vm::Error),
    #[error("Register error")]
    Register,
}

impl From<Error> for TargetError<Error> {
    fn from(e: Error) -> Self {
        TargetError::Fatal(e)
    }
}

struct DebugVm {
    inner: Vm,
    port: u16,
    stream: TcpStream,
    breakpoints: HashMap<PhysAddr, u8>,
}

impl DebugVm {
    pub fn new(inner: Vm, port: u16) -> Self {
        Self {
            inner,
            port,
            stream: wait_for_gdb_connection(port).unwrap(),
            breakpoints: HashMap::new(),
        }
    }
}

impl Target for DebugVm {
    type Arch = gdbstub_arch::x86::X86_64_SSE;
    type Error = crate::vm::Error;

    #[inline(always)]
    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::SingleThread(self)
    }

    fn guard_rail_implicit_sw_breakpoints(&self) -> bool {
        todo!()
    }

    fn use_no_ack_mode(&self) -> bool {
        todo!()
    }

    fn use_x_upcase_packet(&self) -> bool {
        todo!()
    }

    fn use_resume_stub(&self) -> bool {
        todo!()
    }

    fn use_rle(&self) -> bool {
        todo!()
    }

    fn use_target_description_xml(&self) -> bool {
        todo!()
    }

    fn use_lldb_register_info(&self) -> bool {
        todo!()
    }

    fn support_breakpoints(&mut self) -> Option<BreakpointsOps<'_, Self>> {
        todo!()
    }

    fn support_monitor_cmd(&mut self) -> Option<MonitorCmdOps<'_, Self>> {
        todo!()
    }

    fn support_extended_mode(&mut self) -> Option<ExtendedModeOps<'_, Self>> {
        todo!()
    }

    fn support_section_offsets(&mut self) -> Option<SectionOffsetsOps<'_, Self>> {
        todo!()
    }

    fn support_tracepoints(&mut self) -> Option<TracepointsOps<'_, Self>> {
        todo!()
    }

    fn support_target_description_xml_override(
        &mut self,
    ) -> Option<TargetDescriptionXmlOverrideOps<'_, Self>> {
        todo!()
    }

    fn support_lldb_register_info_override(
        &mut self,
    ) -> Option<LldbRegisterInfoOverrideOps<'_, Self>> {
        todo!()
    }

    fn support_memory_map(&mut self) -> Option<MemoryMapOps<'_, Self>> {
        todo!()
    }

    fn support_flash_operations(&mut self) -> Option<FlashOps<'_, Self>> {
        todo!()
    }

    fn support_catch_syscalls(&mut self) -> Option<CatchSyscallsOps<'_, Self>> {
        todo!()
    }

    fn support_host_io(&mut self) -> Option<HostIoOps<'_, Self>> {
        todo!()
    }

    fn support_exec_file(&mut self) -> Option<ExecFileOps<'_, Self>> {
        todo!()
    }

    fn support_auxv(&mut self) -> Option<AuxvOps<'_, Self>> {
        todo!()
    }

    fn support_libraries_svr4(&mut self) -> Option<LibrariesSvr4Ops<'_, Self>> {
        todo!()
    }
}

impl SingleThreadBase for DebugVm {
    fn read_registers(&mut self, output: &mut X86_64CoreRegs) -> TargetResult<(), Self> {
        let (regs, sregs) = self
            .inner
            .vcpu
            .get_all_regs()
            .map_err(|_| TargetError::NonFatal)?;
        // RAX, RBX, RCX, RDX, RSI, RDI, RBP, RSP, r8-r15
        output.regs = [
            regs.rax, regs.rbx, regs.rcx, regs.rdx, regs.rsi, regs.rdi, regs.rbp, regs.rsp,
            regs.r8, regs.r9, regs.r10, regs.r11, regs.r12, regs.r13, regs.r14, regs.r15,
        ];

        output.rip = regs.rip;
        output.eflags = regs.rflags as u32;
        output.segments = X86SegmentRegs {
            cs: sregs.cs.base as u32,
            ss: sregs.ss.base as u32,
            ds: sregs.ds.base as u32,
            es: sregs.es.base as u32,
            fs: sregs.fs.base as u32,
            gs: sregs.gs.base as u32,
        };

        Ok(())
    }

    fn write_registers(&mut self, input: &X86_64CoreRegs) -> TargetResult<(), Self> {
        self.inner.vcpu.mutate_regs(|regs| {
            regs.rip = input.rip;
            regs.rax = input.regs[0];
            regs.rbx = input.regs[1];
            regs.rcx = input.regs[2];
            regs.rdx = input.regs[3];
            regs.rsi = input.regs[4];
            regs.rdi = input.regs[5];
            regs.rbp = input.regs[6];
            regs.rsp = input.regs[7];
            regs.r8 = input.regs[8];
            regs.r9 = input.regs[9];
            regs.r10 = input.regs[10];
            regs.r11 = input.regs[11];
            regs.r12 = input.regs[12];
            regs.r13 = input.regs[13];
            regs.r14 = input.regs[14];
            regs.r15 = input.regs[15];

            true
        });

        Ok(())
    }

    fn support_single_register_access(&mut self) -> Option<SingleRegisterAccessOps<'_, (), Self>> {
        Some(self)
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let addr = VirtAddr::try_new(start_addr).map_err(|_| TargetError::NonFatal)?;

        Ok(0)
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as Arch>::Usize,
        data: &[u8],
    ) -> TargetResult<(), Self> {
        todo!()
    }

    fn support_resume(&mut self) -> Option<SingleThreadResumeOps<'_, Self>> {
        todo!()
    }
}

impl SingleRegisterAccess<()> for DebugVm {
    fn read_register(
        &mut self,
        _tid: (),
        reg_id: X86_64CoreRegId,
        buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let regs = self
            .inner
            .vcpu
            .get_regs()
            .map_err(|_| TargetError::NonFatal)?;
        let val = match reg_id {
            // RAX, RBX, RCX, RDX, RSI, RDI, RBP, RSP, r8-r15
            X86_64CoreRegId::Gpr(id) => match id {
                0 => regs.rax,
                1 => regs.rbx,
                2 => regs.rcx,
                3 => regs.rdx,
                4 => regs.rsi,
                5 => regs.rdi,
                6 => regs.rbp,
                7 => regs.rsp,
                8 => regs.r8,
                9 => regs.r9,
                10 => regs.r10,
                11 => regs.r11,
                12 => regs.r12,
                13 => regs.r13,
                14 => regs.r14,
                15 => regs.r15,
                _ => return Err(TargetError::NonFatal),
            },
            X86_64CoreRegId::Rip => regs.rip,
            X86_64CoreRegId::Eflags => regs.rflags,
            X86_64CoreRegId::Segment(_) => return Err(TargetError::NonFatal),
            X86_64CoreRegId::St(_) => return Err(TargetError::NonFatal),
            X86_64CoreRegId::Fpu(_) => return Err(TargetError::NonFatal),
            X86_64CoreRegId::Xmm(_) => return Err(TargetError::NonFatal),
            X86_64CoreRegId::Mxcsr => return Err(TargetError::NonFatal),
            _ => return Err(TargetError::NonFatal),
        };

        buf.copy_from_slice(&val.to_le_bytes());

        Ok(8)
    }

    fn write_register(
        &mut self,
        _tid: (),
        reg_id: X86_64CoreRegId,
        val: &[u8],
    ) -> TargetResult<(), Self> {
        if val.len() != 8 {
            return Err(TargetError::NonFatal);
        }

        let buffer = val.try_into();
        if buffer.is_err() {
            return Err(TargetError::NonFatal);
        }
        let value = u64::from_le_bytes(buffer.unwrap());
        let mut result = Ok(());

        let regs = self
            .inner
            .vcpu
            .get_regs()
            .map_err(|_| TargetError::NonFatal)?;
        self.inner.vcpu.mutate_regs(|regs| {
            match reg_id {
                // RAX, RBX, RCX, RDX, RSI, RDI, RBP, RSP, r8-r15
                X86_64CoreRegId::Gpr(id) => match id {
                    0 => regs.rax = value,
                    1 => regs.rbx = value,
                    2 => regs.rcx = value,
                    3 => regs.rdx = value,
                    4 => regs.rsi = value,
                    5 => regs.rdi = value,
                    6 => regs.rbp = value,
                    7 => regs.rsp = value,
                    8 => regs.r8 = value,
                    9 => regs.r9 = value,
                    10 => regs.r10 = value,
                    11 => regs.r11 = value,
                    12 => regs.r12 = value,
                    13 => regs.r13 = value,
                    14 => regs.r14 = value,
                    15 => regs.r15 = value,
                    _ => {
                        result = Err(TargetError::NonFatal);
                        return false;
                    }
                },
                X86_64CoreRegId::Rip => regs.rip = value,
                X86_64CoreRegId::Eflags => regs.rflags = value,
                _ => {
                    result = Err(TargetError::NonFatal);
                    return false;
                }
            };
            return true;
        });

        result
    }
}

fn wait_for_gdb_connection(port: u16) -> io::Result<TcpStream> {
    let sockaddr = format!("localhost:{}", port);
    eprintln!("Waiting for a GDB connection on {:?}...", sockaddr);
    let sock = TcpListener::bind(sockaddr)?;
    let (stream, addr) = sock.accept()?;

    // Blocks until a GDB client connects via TCP.
    // i.e: Running `target remote localhost:<port>` from the GDB prompt.

    eprintln!("Debugger connected from {}", addr);
    Ok(stream) // `TcpStream` implements `gdbstub::Connection`
}
