use crate::alloc::*;

use anyhow::anyhow;
use bmvm_common::mem::{
    Align, DefaultAlign, Flags, LayoutTable, LayoutTableEntry, PhysAddr, align_ceil, align_floor,
};
use goblin::elf;
use goblin::elf::{Elf, Header, ProgramHeader};
use goblin::elf32::header::machine_to_str;
use std::fmt::{Debug, Display, Formatter};
use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

macro_rules! info {
    ($($arg:tt)*) => {
        log::info!(target: "guest", $($arg)*);
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        log::error!(target: "guest", $($arg)*);
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

pub struct ExecBundle {
    pub mem_regions: Vec<Region<ReadWrite>>,
    pub entry_point: u64,
    pub layout: Vec<LayoutTableEntry>,
    // pub calls: Vec<CallMeta>,
}

impl ExecBundle {
    pub fn new<P: AsRef<Path>>(path: P, manager: &Manager) -> anyhow::Result<ExecBundle> {
        // early exit if minimal requirements are not met
        check_minimal_file_requirements(&path)?;
        let elf_buf = fs::read(&path)?;

        // early exit if the platform is not supported
        check_platform_supported(&elf_buf)?;
        // parse guest
        let elf = Elf::parse(&elf_buf)?;

        let mut mem_regions: Vec<Region<ReadWrite>> = Vec::new();
        let mut layout: Vec<LayoutTableEntry> = Vec::new();

        // | code | data | heap | ...
        // iterate through all PH_LOAD header and build buffer
        for ph in elf.program_headers.iter() {
            // Skip non PT_LOAD header
            if ph.p_type != elf::program_header::PT_LOAD {
                continue;
            }

            // calc how many pages to allocate
            let p_start = align_floor(ph.p_vaddr);
            let p_end = align_ceil(ph.p_vaddr + ph.p_memsz);
            let to_alloc = p_end - p_start;

            // try creating a layout entry for this segment
            layout.push(Self::build_layout_table_entry(ph, to_alloc, &elf)?);

            // allocate + copy file content to region
            let req_capacity = NonZeroUsize::new(to_alloc as usize).unwrap();
            let mut mem = manager.allocate::<ReadWrite>(req_capacity)?;
            let to_cpy =
                &elf_buf[ph.p_offset as usize..(ph.p_offset as usize + ph.p_filesz as usize)];
            let region_offset = ph.p_vaddr - p_start;
            mem.write_offset(region_offset as usize, to_cpy)?;
            mem.set_guest_addr(PhysAddr::new(p_start));
            mem_regions.push(mem);
        }

        Ok(ExecBundle {
            mem_regions,
            entry_point: elf.header.e_entry,
            layout, // calls: Self::parse_meta(&elf, &elf_buf)?,
        })
    }

    /*
    fn parse_meta(elf: &Elf, buf: &[u8]) -> anyhow::Result<Vec<CallMeta>> {
        let mut content: &[u8] = &[];

        for section in  elf.section_headers.iter() {
            let name_offset = section.sh_name;
            // No index in shstrtab -> no name
            if name_offset == 0 {
                continue;
            }
            // retrieve name from shstrtab and match it with our custom Metadata section name
            if let Some(name) = elf.shdr_strtab.get_at(name_offset) {
                if !name.eq(BMVM_META_SECTION) {
                    continue;
                }

                // retrieve content of section
                content = &buf[section.sh_offset as usize..(section.sh_offset + section.sh_size) as usize];
                break;
            }
        }

        if content.is_empty() {
            return Err(anyhow!("No metadata section found."));
        }

        Ok(CallMeta::try_from_bytes_vec(&content)?)
    }
     */

    fn build_layout_table_entry(
        ph: &ProgramHeader,
        allocated_size: u64,
        elf: &Elf,
    ) -> anyhow::Result<LayoutTableEntry> {
        let p_start = ph.p_vaddr;
        let p_end = ph.p_vaddr + ph.p_memsz;

        // get segment -> section association and create entry in layout table
        for sh in elf.section_headers.iter() {
            // skip empty sections
            if sh.sh_size == 0 {
                continue;
            }

            let s_start = sh.sh_addr;
            let s_end = sh.sh_addr + sh.sh_size;
            if s_start >= p_start && s_end <= p_end {
                let name = elf
                    .shdr_strtab
                    .get_at(sh.sh_name)
                    .ok_or(anyhow!("unknown section"))?;
                let flags = Flags::try_from(name).map_err(|e| anyhow!(e))?;
                let size = allocated_size / DefaultAlign::ALIGNMENT;
                if size > u16::MAX as u64 {
                    return Err(anyhow!(
                        "section {} too large: got {}, max is {}",
                        name,
                        size,
                        u16::MAX
                    ));
                }

                return Ok(LayoutTableEntry::new(
                    PhysAddr::new(p_start),
                    size as u32,
                    flags | Flags::PRESENT,
                ));
            }
        }

        Err(anyhow!("no section found for segment"))
    }
}

fn check_minimal_file_requirements<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let file_meta = match path.as_ref().metadata() {
        Ok(meta) => meta,
        Err(err) => return Err(anyhow!(LoadError::IO(err))),
    };

    if !file_meta.is_file() {
        return Err(anyhow!(LoadError::NotAFile(
            path.as_ref().to_str().unwrap().to_string()
        )));
    }

    // for 32bit systems: guest header + one program header must be at least present
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

fn check_platform_supported<B: AsRef<[u8]>>(buf: B) -> anyhow::Result<()> {
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
