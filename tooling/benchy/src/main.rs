use clap::{Parser, ValueEnum};
use std::path::PathBuf;

mod bench;
mod eval;

#[derive(ValueEnum, Copy, Clone, Debug, Eq, PartialEq)]
enum Runtime {
    Native,
    Wasm,
    Bmvm,
}

#[derive(ValueEnum, Copy, Clone, Debug, Eq, PartialEq)]
enum Mode {
    Start,
    Exec,
}

impl Mode {
    fn dir(&self) -> String {
        match self {
            Mode::Start => String::from("startup"),
            Mode::Exec => String::from("exec"),
        }
    }
}

impl Runtime {
    fn exec(&self, path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
        match self {
            Runtime::Native => bench::exec::native(path, warmup, iters),
            Runtime::Wasm => bench::exec::wasm(path, warmup, iters),
            Runtime::Bmvm => bench::exec::bmvm(path, warmup, iters),
        }
    }

    fn startup(&self, path: &PathBuf, warmup: usize, iters: usize) -> anyhow::Result<Vec<f64>> {
        match self {
            Runtime::Wasm => bench::startup::wasm(path, warmup, iters),
            Runtime::Bmvm => bench::startup::bmvm(path, warmup, iters),
            _ => Err(anyhow::anyhow!(
                "Startup is not supported for this runtime: {self:?}"
            )),
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

fn validate(args: &Args) -> anyhow::Result<()> {
    if args.runtime == Runtime::Native && args.mode == Mode::Start {
        return Err(anyhow::anyhow!(
            "Native runtime does not support start mode"
        ));
    }

    if !args.file.is_file() {
        return Err(anyhow::anyhow!(
            "Provided path is not a file: {}",
            args.file.display()
        ));
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, env = "FILE")]
    file: PathBuf,
    #[arg(short, long, env = "RUNTIME")]
    runtime: Runtime,
    #[arg(short, long, env = "MODE", default_value = "exec")]
    mode: Mode,
    #[arg(short, long, env = "WARMUP", default_value = "0")]
    warmup: usize,
    #[arg(short, long, env = "ITERATIONS", default_value = "50")]
    iters: usize,
    #[arg(short, long, env = "OUTPUT")]
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    validate(&args)?;

    let mut output = if let Some(output) = args.output {
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

    let results = match args.mode {
        Mode::Start => args.runtime.startup(&args.file, args.warmup, args.iters)?,
        Mode::Exec => args.runtime.exec(&args.file, args.warmup, args.iters)?,
    };

    output.push(args.mode.dir());
    output.push(args.runtime.dir());
    output.push(args.file.file_stem().unwrap());

    eval::eval(output, &results)
}
