use bmvm_common::interprete::Interpret;
use bmvm_common::mem::{Flags, LayoutTable};
use clap::Parser;
use std::fs;
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct TableEntry {
    idx: usize,
    paddr: String,
    vaddr: String,
    size: usize,
    stack: bool,
    data_usage: String,
    code: bool,
    system: bool,
    present: bool,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "FILE")]
    file: String,

    #[arg(short, long, env = "OFFSET", default_value_t = 0)]
    offset: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let dump = fs::read(args.file)?;
    let table = LayoutTable::from_bytes(&dump[args.offset..])?;

    let mut table_entries = Vec::new();

    for (idx, entry) in table.into_iter().enumerate() {
        let access = match entry.flags().data_access_mode() {
            Some(a) => format!("{}", a),
            None => "N/A".to_string(),
        };

        table_entries.push(TableEntry {
            idx,
            paddr: format!("{:X}", entry.paddr().as_u64() as usize),
            vaddr: format!("{:X}", entry.vaddr().as_u64() as usize),
            size: entry.pages() as usize,
            stack: entry.flags().contains(Flags::STACK),
            data_usage: access,
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
