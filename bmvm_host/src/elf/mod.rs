use crate::alloc::*;

use anyhow::anyhow;
use goblin::elf;
use goblin::elf::{Elf, Header};
use goblin::elf32::header::machine_to_str;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::path::Path;

macro_rules! info {
    ($($arg:tt)*) => {
        log::info!(target: "elf", $($arg)*);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        log::error!(target: "elf", $($arg)*);
    };
}

const SUPPORTED_PLATFORMS: &[u16] = &[elf::header::EM_386, elf::header::EM_X86_64];

#[derive(Debug)]
pub enum LoadError {
    IO(std::io::Error),
    NotAFile(String),
    FileTooSmall(String, usize, usize),
    UnsupportedPlatform(u16),
    ParseError(goblin::error::Error),
}

impl Display for LoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NotAFile(path) => write!(f, "Not a file: {}", path),
            LoadError::FileTooSmall(path, expected, actual) => write!(
                f,
                "file at {} too small (expected: {}, actual: {})",
                path, expected, actual
            ),
            LoadError::UnsupportedPlatform(machine) => {
                write!(
                    f,
                    "Unsupported machine: {}. Must be one of {}",
                    machine_to_str(*machine),
                    SUPPORTED_PLATFORMS
                        .iter()
                        .map(|x| machine_to_str(*x))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            LoadError::ParseError(e) => write!(f, "Unable to parse ELF: {}", e),
            LoadError::IO(err) => write!(f, "IO: {}", err),
        }
    }
}

impl std::error::Error for LoadError {}

pub struct CallMeta {
    id: u32,
    name: String,
}

pub struct ExecBundle {
    pub mem_regions: Vec<Region<ReadWrite>>,
    pub entry_point: u64,
    pub stack_pointer: u64,
}

impl ExecBundle {
    pub fn new<P: AsRef<Path>>(path: P, manager: impl Manager) -> anyhow::Result<ExecBundle> {
        let elf_buf = fs::read(path)?;
        let elf = Elf::parse(&elf_buf)?;

        let mut mem_regions: Vec<Region<ReadWrite>> = Vec::new();

        // stack should start from here
        // | code | data | stack |
        // 0 assumes no PH_LOAD header were provided and therefore starts at the very beginning
        // a separate memory region should be allocated for the stack
        // this will be done in the vm execution function
        let mut next_page = 0u64;

        // iterate through all PH_LOAD header and build buffer
        for ph in elf.program_headers.iter() {
            // Skip non PT_LOAD header
            if ph.p_type != elf::program_header::PT_LOAD {
                continue;
            }

            // calc how many pages to allocate
            let p_start = DefaultAlign::align_floor(ph.p_vaddr);
            let mut p_end = DefaultAlign::align_ceil(ph.p_vaddr + ph.p_memsz);
            p_end = DefaultAlign::align_ceil(p_end);
            let to_alloc = p_end - p_start;

            // allocate + copy file content to region
            let mut mem = manager.allocate::<ReadWrite, DefaultAlign>(to_alloc)?;
            let to_cpy =
                &elf_buf[ph.p_offset as usize..(ph.p_offset as usize + ph.p_filesz as usize)];
            let region_offset = ph.p_vaddr - p_start;
            mem.write_offset(region_offset as usize, to_cpy)?;

            // save region for use in bundle
            // mem.set_guest_addr(p_start);
            mem_regions.push(mem);

            // save next possible mem region for later use
            next_page = p_end;
        }

        Ok(ExecBundle {
            mem_regions,
            entry_point: elf.header.e_entry,
            stack_pointer: next_page,
        })
    }
    
    fn parse_meta() -> anyhow::Result<()> {
        Ok(())
    }
}

pub(crate) fn check_minimal_file_requirements<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let file_meta = match path.as_ref().metadata() {
        Ok(meta) => meta,
        Err(err) => return Err(anyhow!(LoadError::IO(err))),
    };

    if !file_meta.is_file() {
        return Err(anyhow!(LoadError::NotAFile(
            path.as_ref().to_str().unwrap().to_string()
        )));
    }

    // for 32bit systems: elf header + one program header must be at least present
    let min_size =
        elf::header::header32::SIZEOF_EHDR + elf::program_header::program_header32::SIZEOF_PHDR;
    if file_meta.len() < min_size as u64 {
        return Err(anyhow!(LoadError::FileTooSmall(
            path.as_ref().to_str().unwrap().to_string(),
            min_size,
            file_meta.len() as usize,
        )));
    }

    Ok(())
}

pub(crate) fn check_platform_supported<B: AsRef<[u8]>>(buf: B) -> anyhow::Result<()> {
    let header: Header;
    match Elf::parse_header(buf.as_ref()) {
        Ok(result) => {
            header = result;
        }
        Err(err) => return Err(anyhow!(LoadError::ParseError(err))),
    }

    if !SUPPORTED_PLATFORMS.contains(&header.e_machine) {
        return Err(anyhow!(LoadError::UnsupportedPlatform(header.e_machine)));
    }

    Ok(())
}
