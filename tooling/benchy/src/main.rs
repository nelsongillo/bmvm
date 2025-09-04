use clap::{Parser, ValueEnum};
use std::path::PathBuf;

mod eval;
mod exec;

#[derive(ValueEnum, Copy, Clone, Debug)]
enum Runtime {
    Native,
    Wasm,
    Bmvm,
}

impl Runtime {
    fn exec(&self, path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
        match self {
            Runtime::Native => exec::native(path, warmup, iters),
            Runtime::Wasm => exec::wasm(path, warmup, iters),
            Runtime::Bmvm => exec::bmvm(path, warmup, iters),
        }
    }

    fn dir(&self) -> String {
        match self {
            Runtime::Native => String::from("native"),
            Runtime::Wasm => String::from("wasm"),
            Runtime::Bmvm => String::from("bmvm"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "FILE")]
    file: PathBuf,
    #[arg(short, long, env = "RUNTIME")]
    runtime: Runtime,
    #[arg(short, long, env = "WARMUP", default_value = "0")]
    warmup: usize,
    #[arg(short, long, env = "ITERATIONS", default_value = "50")]
    iters: usize,
    #[arg(short, long, env = "OUTPUT")]
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.file.is_file() {
        return Err(anyhow::anyhow!(
            "Provided path is not a file: {}",
            args.file.display()
        ));
    }

    let mut output = if let Some(output) = args.output {
        let o = PathBuf::from(output);
        if !o.is_dir() {
            return Err(anyhow::anyhow!(
                "Provided output path is not a directory: {}",
                o.display()
            ));
        }
        o
    } else {
        PathBuf::from(".")
    };

    let results = args.runtime.exec(&args.file, args.warmup, args.iters)?;

    output.push(args.runtime.dir());
    output.push(args.file.file_stem().unwrap());

    eval::eval(output, &results)
}
