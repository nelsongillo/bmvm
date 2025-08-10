const ENV_DUMP_FILE: &str = "FILE";
const ENV_OFFSET: &str = "OFFSET";

use bmvm_common::interprete::Interpret;
use bmvm_common::mem::{Flags, LayoutTable};
use clap::Parser;
use std::fmt::Display;
use std::fs;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Debug)]
enum DataUsage {
    NotData,
    Read,
    Write,
    OwnedShared,
    ForeignShared,
    Unknown,
}

impl From<Flags> for DataUsage {
    fn from(flags: Flags) -> Self {
        if !flags.contains(Flags::DATA) {
            return DataUsage::NotData;
        }

        match () {
            _ if flags.contains(Flags::READ) => DataUsage::Read,
            _ if flags.contains(Flags::WRITE) => DataUsage::Write,
            _ if flags.contains(Flags::SHARED_FOREIGN) => DataUsage::ForeignShared,
            _ if flags.contains(Flags::SHARED_OWNED) => DataUsage::OwnedShared,
            _ => DataUsage::Unknown,
        }
    }
}

impl Display for DataUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Tabled)]
struct TableEntry {
    idx: usize,
    addr: String,
    size: usize,
    stack: bool,
    data_usage: DataUsage,
    code: bool,
    system: bool,
    present: bool,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = ENV_DUMP_FILE)]
    file: String,

    #[arg(short, long, env = ENV_OFFSET, default_value_t = 0)]
    offset: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let dump = fs::read(args.file)?;
    let table = LayoutTable::from_bytes(&dump[args.offset..])?;

    let mut table_entries = Vec::new();

    for (idx, entry) in table.into_iter().enumerate() {
        table_entries.push(TableEntry {
            idx,
            addr: format!("{:X}", entry.addr().as_u64() as usize),
            size: entry.len() as usize,
            stack: entry.flags().contains(Flags::STACK),
            data_usage: DataUsage::from(entry.flags()),
            code: entry.flags().contains(Flags::CODE),
            system: entry.flags().contains(Flags::SYSTEM),
            present: entry.flags().contains(Flags::PRESENT),
        });
    }

    let mut table = Table::new(table_entries);
    table.with(Style::modern());
    println!("{}", table);

    Ok(())
}
