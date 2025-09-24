use indicatif::ProgressBar;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

pub mod exec;
pub mod startup;

type Pre<I, T> = fn(I) -> anyhow::Result<T>;
type Exec<T> = fn(&mut T) -> anyhow::Result<f64>;
type Post<T> = fn(&mut T) -> anyhow::Result<()>;

fn bench<I, T>(
    input: I,
    warmup: usize,
    iters: usize,
    prep: Pre<I, T>,
    exec: Exec<T>,
    post: Post<T>,
) -> anyhow::Result<Vec<f64>>
where
    I: Debug,
{
    let mut samples: Vec<f64> = Vec::with_capacity(iters);
    println!("Executable: {:?}", input);

    let mut state = prep(input)?;

    // Executing optional warmup phase
    if warmup > 0 {
        println!("Warmup...");
        let bar = ProgressBar::new(warmup as u64);
        bar.set_position(0);
        for i in 0..warmup {
            let _ = exec(&mut state)?;
            bar.inc(i as u64 + 1);
        }
        bar.finish();
    }

    // Executing Sampling
    println!("Sampling...");
    let bar = ProgressBar::new(iters as u64);
    bar.set_position(0);
    for i in 0..iters {
        let sample = exec(&mut state)?;
        samples.push(sample);
        bar.set_position(i as u64 + 1);
    }
    bar.finish();
    println!("Execution Finished.");

    post(&mut state)?;

    Ok(samples)
}
