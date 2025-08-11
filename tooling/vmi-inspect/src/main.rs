use anyhow::anyhow;
use bmvm_common::vmi::{FnCall, FnPtr, UpcallFn};
use bmvm_common::{
    BMVM_META_SECTION_DEBUG, BMVM_META_SECTION_EXPOSE, BMVM_META_SECTION_EXPOSE_CALLS,
    BMVM_META_SECTION_HOST,
};
use clap::Parser;
use goblin::elf::Elf;
use std::cmp::max;
use std::ffi::CString;
use std::fs;
use tabled::builder::Builder;
use tabled::settings::{Panel, Style};
use tabled::{Table, Tabled};

#[derive(Debug)]
struct VmiInfo {
    debug: bool,
    expose: Vec<FnCall>,
    upcalls: Vec<UpcallFn>,
    /// All function calls expected to be provided to the guest by the host.
    /// The vector is guaranteed to be sorted.
    host: Vec<FnCall>,
}

impl VmiInfo {
    fn new(buf: &[u8]) -> anyhow::Result<Self> {
        let elf = Elf::parse(buf.as_ref())?;
        let debug = Self::is_vmi_debug(&elf);
        let host = Self::parse_vmi_vec(&elf, &buf, BMVM_META_SECTION_HOST, debug)?;
        let expose = Self::parse_vmi_vec(&elf, &buf, BMVM_META_SECTION_EXPOSE, debug)?;
        let upcalls = if !expose.is_empty() {
            Self::parse_upcall_ptr(&elf, &buf, BMVM_META_SECTION_EXPOSE_CALLS, expose.len())?
        } else {
            Vec::new()
        };

        Ok(Self {
            debug,
            expose,
            upcalls,
            host,
        })
    }

    /// If the debug section header is included, then VMI call data includes debug information
    /// i.e. parameter and return types
    fn is_vmi_debug(elf: &Elf) -> bool {
        Self::find_section_header(elf, BMVM_META_SECTION_DEBUG).is_some()
    }

    /// Return the index to the section header if a section with `name` is found in the ELF file
    fn find_section_header(elf: &Elf, name: &str) -> Option<usize> {
        for (idx, section) in elf.section_headers.iter().enumerate() {
            let name_offset = section.sh_name;
            // No index in shstrtab -> no name
            if name_offset == 0 {
                continue;
            }
            // retrieve name from shstrtab and match
            if let Some(sh_name) = elf.shdr_strtab.get_at(name_offset) {
                if name.eq(sh_name) {
                    return Some(idx);
                }
            }
        }

        None
    }

    /// Parse a vector of VMI calls from the ELF file from the section with `name`.
    /// On success, the returned vector is sorted via Vec::sort
    fn parse_vmi_vec(
        elf: &Elf,
        buf: &[u8],
        section_name: &str,
        debug: bool,
    ) -> anyhow::Result<Vec<FnCall>> {
        if let Some(idx) = Self::find_section_header(elf, section_name) {
            let section = &elf.section_headers[idx];
            let content =
                &buf[section.sh_offset as usize..(section.sh_offset + section.sh_size) as usize];

            if content.is_empty() {
                log::warn!("VMI section defined but empty: {}", section_name);
                return Ok(Vec::new());
            }

            let mut calls = FnCall::try_from_bytes_vec(content, debug)
                .map_err(|e| anyhow!("Error parsing VMI section '{}': {}", section_name, e))?;
            // ensure to sort the function calls
            calls.sort();
            return Ok(calls);
        }

        Ok(Vec::new())
    }

    fn parse_upcall_ptr(
        elf: &Elf,
        buf: &[u8],
        section_name: &str,
        count: usize,
    ) -> anyhow::Result<Vec<UpcallFn>> {
        if let Some(idx) = Self::find_section_header(elf, section_name) {
            let section = &elf.section_headers[idx];
            let content =
                &buf[section.sh_offset as usize..(section.sh_offset + section.sh_size) as usize];

            let size = size_of::<UpcallFn>();
            if content.len() < count * size {
                return Err(anyhow!(
                    "Insufficient upcall pointers: want {} but got {}",
                    count,
                    content.len() / size
                ));
            }

            let mut calls = UpcallFn::try_from_bytes_vec(content)
                .map_err(|e| anyhow!("Error parsing VMI section '{}': {}", section_name, e))?;
            // ensure to sort the function calls
            calls.sort();
            return Ok(calls);
        }

        Ok(Vec::new())
    }

    fn table_expose(&self) -> anyhow::Result<Table> {
        let mut builder = Builder::default();

        let psize = Self::required_param_columns(&self.expose);
        let cols = 1 + 1 + psize + 1 + 1;
        let mut columns = Vec::with_capacity(cols);
        columns.push("Signature");
        columns.push("Name");
        for i in 0..psize {
            columns.push("Param");
        }
        columns.push("Return");
        columns.push("Ptr");
        builder.push_record(columns);

        for func in self.expose.iter() {
            for ptr in self.upcalls.iter() {
                if func.sig == ptr.sig {
                    let mut row = Vec::with_capacity(cols);
                    row.push(func.sig.to_string());
                    row.push(func.name.clone().into_string()?);
                    row.extend(
                        func.debug_param_types
                            .iter()
                            .map(|c| c.to_owned().into_string().unwrap()),
                    );
                    let output = func
                        .debug_return_type
                        .clone()
                        .map(|c| c.to_owned().into_string().unwrap())
                        .unwrap_or_else(|| "()".to_string());
                    row.push(output);
                    row.push(ptr.func.as_u64().to_string());

                    builder.push_record(row);
                }
            }
        }

        let mut table = builder.build();
        table.with(Style::modern());
        table.with(Panel::header("Upcalls"));
        Ok(table)
    }

    fn table_host(&self) -> anyhow::Result<Table> {
        let mut builder = Builder::default();

        let psize = Self::required_param_columns(&self.host);
        let cols = 1 + 1 + psize + 1;
        let mut columns = Vec::with_capacity(cols);
        columns.push("Signature");
        columns.push("Name");
        for i in 0..psize {
            columns.push("Param");
        }
        columns.push("Return");
        builder.push_record(columns);

        for func in self.host.iter() {
            let mut row = Vec::with_capacity(cols);
            row.push(func.sig.to_string());
            row.push(func.name.clone().into_string()?);
            row.extend(
                func.debug_param_types
                    .iter()
                    .map(|c| c.to_owned().into_string().unwrap()),
            );
            let output = func
                .debug_return_type
                .clone()
                .map(|c| c.to_owned().into_string().unwrap())
                .unwrap_or_else(|| "()".to_string());
            row.push(output);

            builder.push_record(row);
        }

        let mut table = builder.build();
        table.with(Style::modern());
        table.with(Panel::header("Hypercalls"));
        Ok(table)
    }

    fn required_param_columns(calls: &Vec<FnCall>) -> usize {
        calls.iter().map(|r| r.params().len()).max().unwrap_or(0)
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "FILE")]
    file: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let dump = fs::read(args.file)?;

    let info = VmiInfo::new(&dump)?;
    println!("{}\n", info.table_expose()?);
    println!("{}", info.table_host()?);

    Ok(())
}
