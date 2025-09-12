use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Summary {
    mean: f64,
    std: f64,
}

#[derive(Debug)]
pub struct PlotData {
    data: HashMap<String, (u64, u64)>,
}

pub fn collect_data(data_dir: &Path) -> Result<PlotData> {
    let mut data: HashMap<String, (u64, u64)> = HashMap::new();

    // First, collect all types (directories) and ensure we have consistent executions
    let dir_entries: Vec<_> = fs::read_dir(data_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.metadata().map(|m| m.is_dir()).unwrap_or(false))
        .collect();

    // Now collect data for each type
    for entry in dir_entries {
        let type_name = entry.file_name().into_string().unwrap_or_default();

        let summary_path = entry.path().join("summary.json");
        if !summary_path.exists() {
            anyhow::bail!("Missing summary.json for {}", type_name);
        }

        let summary_content = fs::read_to_string(&summary_path)
            .with_context(|| format!("Failed to read {:?}", summary_path))?;

        let summary: Summary = serde_json::from_str(&summary_content)
            .with_context(|| format!("Failed to parse JSON in {:?}", summary_path))?;

        let mean = summary.mean.floor() as u64;
        let stddev = summary.std.floor() as u64;

        data.insert(type_name, (mean, stddev));
    }

    Ok(PlotData { data })
}

pub fn generate_latex_plot(plot_data: &PlotData, output: &Path) -> Result<()> {
    let mut latex = String::new();

    // LaTeX document preamble
    latex.push_str(
        r#"
\documentclass{standalone}
\usepackage{pgfplots}
\pgfplotsset{compat=1.18}
\usepackage{textcomp}
\usepackage{amsmath}

\begin{document}
\begin{tikzpicture}
\begin{axis}[
    ybar,
    bar width=15pt,
    ymin=0,
    ylabel={Startup Time},
    symbolic x coords={1},
    xtick=\empty,
    xticklabels=\empty,
    enlarge x limits=0.5,
    legend style={at={(0.5,-0.15)}, anchor=north, legend columns=-1}
]"#,
    );

    // Add plot for each type
    for (mean, stddev) in plot_data.data.values() {
        latex.push_str(
            r#"
\addplot+[
    error bars/.cd,
    y dir=both,
    y explicit
] coordinates {"#,
        );
        latex.push_str(&format!("(1,{}) +- (0,{})}};\n", mean, stddev));
    }

    let legend = plot_data
        .data
        .keys()
        .cloned()
        .collect::<Vec<String>>()
        .join(",");
    latex.push_str(format!("\\legend{{{}}}\n", legend).as_str());

    // Close the axis and document
    latex.push_str(
        r#"
\end{axis}
\end{tikzpicture}
\end{document}
"#,
    );

    fs::write(output, latex)?;
    println!("LaTeX plot generated successfully at: {}", output.display());
    Ok(())
}

pub fn plot(data_dir: &Path, output: &Path) -> Result<()> {
    let mut o = output.to_path_buf();
    o.push("startup.tex");

    println!("Collecting data from: {}", data_dir.display());
    let plot_data = collect_data(data_dir)?;

    println!("Generating LaTeX plot...");
    generate_latex_plot(&plot_data, &o)
}
