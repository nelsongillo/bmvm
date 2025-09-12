mod plot;

use crate::plot::startup;
use anyhow::Result;
use clap::{Parser, ValueEnum};
use plot::polybench;
use std::path::PathBuf;

#[derive(ValueEnum, Copy, Clone, Debug, Eq, PartialEq)]
enum Benchmark {
    Polybench,
    Startup,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "DIR", default_value = "data")]
    dir: PathBuf,
    #[arg(short, long, env = "BENCHMARK")]
    benchmark: Benchmark,
    #[arg(short, long, env = "OUTPUT")]
    output: Option<String>,
}

fn validate(args: &Args) -> Result<()> {
    if !args.dir.exists() {
        return Err(anyhow::anyhow!(
            "Provided path does not exist: {}",
            args.dir.display()
        ));
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    validate(&args)?;

    let output = if let Some(output) = args.output {
        let o = PathBuf::from(output);
        if o.exists() && !o.is_dir() {
            return Err(anyhow::anyhow!(
                "Provided output path is not a directory: {}",
                o.display()
            ));
        }
        o
    } else {
        PathBuf::from(".")
    };

    match args.benchmark {
        Benchmark::Polybench => polybench::plot(args.dir.as_path(), output.as_path()),
        Benchmark::Startup => startup::plot(args.dir.as_path(), output.as_path()),
    }
}
