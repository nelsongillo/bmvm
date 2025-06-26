use crate::alloc::*;

use bmvm_common::mem::{
    Align, AlignedNonZeroUsize, DefaultAlign, Flags, LayoutTableEntry, MAX_REGION_SIZE, PhysAddr,
    align_ceil, align_floor,
};
use goblin::elf;
use goblin::elf::{Elf, Header, ProgramHeader};
use goblin::elf32::header::machine_to_str;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

#[cfg(target_arch = "x86_64")]
const SUPPORTED_PLATFORMS: &[u16] = &[elf::header::EM_X86_64];

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("path is not a file: {0}")]
    NotAFile(String),

    #[error("file at path {path} is to small: required min {min} but got {size}")]
    FileTooSmall {
        path: String,
        min: usize,
        size: usize,
    },

    #[error("Unsupported machine: {0}")]
    UnsupportedPlatform(&'static str),

    #[error("unknown section at index {0}")]
    ElfUnnamedSection(usize),

    #[error("section {name} too large: got {size} but only supports up to {max}")]
    ElfSectionTooLarge { name: String, max: u64, size: u64 },

    #[error("no associated section found for LOAD segment at index {0}")]
    ElfNoSectionForSegment(usize),

    #[error("unsupported section {0}")]
    ElfUnsupportedSection(String),

    #[error("Invalid entry point: {0}")]
    InvalidEntryPoint(u64),

    #[error("Unable to parse ELF: {0}")]
    ElfParse(#[from] goblin::error::Error),

    #[error("{0}")]
    Alloc(#[from] alloc::Error),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}

pub struct Buffer {
    inner: Vec<u8>,
}

impl Buffer {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        // early exit if minimal requirements are not met
        check_minimal_file_requirements(&path)?;
        let buf = fs::read(&path)?;

        // early exit if the platform is not supported
        check_platform_supported(&buf)?;

        Ok(Self { inner: buf })
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

pub struct ExecBundle {
    pub(crate) entry: PhysAddr,
    pub(crate) mem_regions: RegionCollection,
    pub(crate) layout: Vec<LayoutTableEntry>,
    // pub calls: Vec<CallMeta>,
}

fn section_name_to_flags(name: &str) -> Result<Flags> {
    match name {
        _ if name.starts_with(".text") => Ok(Flags::CODE), // Executable code
        _ if name.starts_with(".rodata") => Ok(Flags::DATA | Flags::READ), // Read-only constants/data
        _ if name.starts_with(".eh_frame") => Ok(Flags::DATA | Flags::READ), // Exception handling tables (read-only)
        _ if name.starts_with(".data") => Ok(Flags::DATA | Flags::WRITE), // Initialized writable data
        _ if name.starts_with(".bss") => Ok(Flags::DATA | Flags::WRITE), // Uninitialized data (zero-filled)
        _ if name.starts_with(".got") => Ok(Flags::DATA | Flags::READ),
        _ => Err(Error::ElfUnsupportedSection(name.to_string())),
    }
}

impl ExecBundle {
    pub(crate) fn from_buffer(buf: Buffer, manager: &Allocator) -> Result<Self> {
        let elf = Elf::parse(buf.as_ref())?;

        let entry =
            PhysAddr::try_from(elf.entry).map_err(|_| Error::InvalidEntryPoint(elf.entry))?;
        let mut layout = Vec::new();
        let mut mem_regions = RegionCollection::new();

        // | code | data | heap | ...
        // iterate through all PH_LOAD header and build buffer
        for (idx, ph) in elf.program_headers.iter().enumerate() {
            // Skip non PT_LOAD header
            if ph.p_type != elf::program_header::PT_LOAD {
                continue;
            }

            // calc how many pages to allocate
            let p_start = align_floor(ph.p_vaddr);
            let p_end = align_ceil(ph.p_vaddr + ph.p_memsz);
            let to_alloc = p_end - p_start;

            // try creating a layout entry for this segment
            layout.push(Self::build_layout_table_entry(idx, ph, to_alloc, &elf)?);

            // allocate + copy file content to region
            let req_capacity = AlignedNonZeroUsize::new_ceil(to_alloc as usize).unwrap();
            let mut proto_mem = manager.alloc_accessible::<ReadWrite>(req_capacity)?;
            let to_cpy =
                &buf.as_ref()[ph.p_offset as usize..(ph.p_offset as usize + ph.p_filesz as usize)];
            let region_offset = ph.p_vaddr - p_start;
            proto_mem.write_offset(region_offset as usize, to_cpy)?;
            let mem = proto_mem.set_guest_addr(PhysAddr::new(p_start));
            mem_regions.push(mem);
        }

        Ok(Self {
            entry,
            mem_regions,
            layout,
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
        ph_idx: usize,
        ph: &ProgramHeader,
        allocated_size: u64,
        elf: &Elf,
    ) -> Result<LayoutTableEntry> {
        let p_start = align_floor(ph.p_vaddr);
        let p_end = align_ceil(ph.p_vaddr + ph.p_memsz);

        // get segment -> section association and create entry in layout table
        for (i, sh) in elf.section_headers.iter().enumerate() {
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
                    .ok_or(Error::ElfUnnamedSection(i))?;
                let flags = section_name_to_flags(name)?;
                if allocated_size > MAX_REGION_SIZE {
                    return Err(Error::ElfSectionTooLarge {
                        name: name.to_string(),
                        max: MAX_REGION_SIZE,
                        size: allocated_size,
                    });
                }

                return Ok(LayoutTableEntry::new(
                    PhysAddr::new(p_start),
                    (allocated_size / DefaultAlign::ALIGNMENT) as u32,
                    flags | Flags::PRESENT,
                ));
            }
        }

        Err(Error::ElfNoSectionForSegment(ph_idx))
    }
}

fn check_minimal_file_requirements<P: AsRef<Path>>(path: P) -> Result<()> {
    let file_meta = path.as_ref().metadata()?;

    if !file_meta.is_file() {
        return Err(Error::NotAFile(path.as_ref().to_str().unwrap().to_string()));
    }

    // for 32bit systems: guest header and one program header must be at least present
    let min_size =
        elf::header::header32::SIZEOF_EHDR + elf::program_header::program_header32::SIZEOF_PHDR;
    if file_meta.len() < min_size as u64 {
        return Err(Error::FileTooSmall {
            path: path.as_ref().to_str().unwrap().to_string(),
            min: min_size,
            size: file_meta.len() as usize,
        });
    }

    Ok(())
}

fn check_platform_supported<B: AsRef<[u8]>>(buf: B) -> Result<()> {
    let header: Header;
    match Elf::parse_header(buf.as_ref()) {
        Ok(result) => {
            header = result;
        }
        Err(err) => return Err(Error::ElfParse(err)),
    }

    if !SUPPORTED_PLATFORMS.contains(&header.e_machine) {
        return Err(Error::UnsupportedPlatform(machine_to_str(header.e_machine)));
    }

    Ok(())
}
