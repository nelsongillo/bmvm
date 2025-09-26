use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const FILE_RAW: &str = "raw.csv";
const FILE_SUMMARY: &str = "summary.json";

#[derive(Serialize)]
struct Summary {
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
    var: f64,
    std: f64,
}

#[repr(transparent)]
struct Samples([f64]);

impl Samples {
    fn new(slice: &[f64]) -> &Samples {
        assert!(slice.len() > 1);
        assert!(&&slice.iter().all(|x| !x.is_nan()));

        unsafe { std::mem::transmute(slice) }
    }

    fn min(&self) -> f64 {
        let head = self.first();
        self.0.iter().fold(*head, |a, &b| a.min(b))
    }

    fn max(&self) -> f64 {
        let head = self.first();
        self.0.iter().fold(*head, |a, &b| a.max(b))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn first(&self) -> &f64 {
        self.0.first().unwrap()
    }

    fn iter(&self) -> impl Iterator<Item = &f64> {
        self.0.iter()
    }

    fn sum(&self) -> f64 {
        self.iter().sum()
    }

    fn mean(&self) -> f64 {
        let n = self.len();

        self.sum() / n as f64
    }

    fn var(&self, mean: Option<f64>) -> f64 {
        let mean = mean.unwrap_or_else(|| self.mean());
        let len = self.len();

        let sum = self
            .iter()
            .map(|&x| (x - mean).powi(2))
            .fold(0f64, std::ops::Add::add);

        sum / (len - 1) as f64
    }

    fn std_dev(&self, mean: Option<f64>) -> f64 {
        self.var(mean).sqrt()
    }

    fn median(&self) -> f64 {
        let mut v = self.0.to_vec();
        v.sort_by(f64::total_cmp);
        let len = v.len();

        if len % 2 == 0 {
            let mid = len / 2;
            (v[mid] + v[mid - 1]) / 2.0
        } else {
            let mid = len / 2;
            v[mid]
        }
    }

    fn summary(&self) -> Summary {
        let min = self.min();
        let max = self.max();
        let mean = self.mean();
        let median = self.median();
        let var = self.var(Some(median));
        let std = self.std_dev(Some(median));

        Summary {
            min,
            max,
            mean,
            median,
            var,
            std,
        }
    }
}

pub fn multi_eval<const N: usize>(
    directory: PathBuf,
    durations: &[Vec<f64>; N],
) -> anyhow::Result<()> {
    println!("Evaluating...");
    println!("Writing results to {}", directory.display());
    for i in 0..N {
        let current = directory.join(i.to_string());
        std::fs::create_dir_all(&current)?;
        let samples = Samples::new(&durations[i]);
        let summary = samples.summary();
        write_raw(&current, samples)?;
        write_summary(&current, &summary)?;
    }

    Ok(())
}

pub fn eval(directory: PathBuf, durations: &[f64]) -> anyhow::Result<()> {
    println!("Evaluating...");
    println!("Writing results to {}", directory.display());
    std::fs::create_dir_all(&directory)?;

    if durations.is_empty() {
        return Err(anyhow::anyhow!("No values to evaluate"));
    }

    let samples = Samples::new(durations);
    let summary = samples.summary();

    write_raw(&directory, samples)?;
    write_summary(&directory, &summary)?;
    Ok(())
}

fn write_summary(path: &PathBuf, summary: &Summary) -> anyhow::Result<()> {
    let file = File::create(path.join(FILE_SUMMARY))?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, summary)?;
    Ok(())
}

/// Write the raw data to file
fn write_raw(path: &PathBuf, samples: &Samples) -> anyhow::Result<()> {
    let file = File::create(path.join(FILE_RAW))?;
    let mut writer = BufWriter::new(file);

    for s in samples.iter() {
        writeln!(writer, "{}", s)?;
    }
    writer.flush()?;

    Ok(())
}
