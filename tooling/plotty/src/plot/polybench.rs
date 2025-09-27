use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Summary {
    mean: f64,
}

#[derive(Debug)]
pub struct PlotData {
    types: Vec<String>,
    executions: Vec<String>,
    data: HashMap<String, Vec<u64>>,
}

pub fn collect_data(data_dir: &Path) -> Result<PlotData> {
    let mut types = Vec::new();
    let mut data: HashMap<String, Vec<u64>> = HashMap::new();
    let mut freq = HashMap::<String, usize>::new();

    // First, collect all types (directories) and ensure we have consistent executions
    let dir_entries: Vec<_> = fs::read_dir(data_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.metadata().map(|m| m.is_dir()).unwrap_or(false))
        .collect();

    for entry in &dir_entries {
        let type_name = entry.file_name().into_string().unwrap_or_default();
        types.push(type_name.clone());

        // Collect executions for this type
        fs::read_dir(entry.path())?
            .filter_map(Result::ok)
            .filter(|e| e.metadata().map(|m| m.is_dir()).unwrap_or(false))
            .map(|e| e.file_name().into_string().unwrap_or_default())
            .for_each(|e| *freq.entry(e).or_insert(0) += 1);
    }

    let mut executions: Vec<String> = freq
        .into_iter()
        .filter(|(_, v)| *v == dir_entries.len())
        .map(|(k, _)| k)
        .collect();

    executions.sort();

    // Now collect data for each type
    for entry in dir_entries {
        let type_name = entry.file_name().into_string().unwrap_or_default();
        let mut mean_values = Vec::new();

        for execution in &executions {
            let summary_path = entry.path().join(execution).join("summary.json");
            if !summary_path.exists() {
                anyhow::bail!("Missing summary.json for {}:{}", type_name, execution);
            }

            let summary_content = fs::read_to_string(&summary_path)
                .with_context(|| format!("Failed to read {:?}", summary_path))?;

            let summary: Summary = serde_json::from_str(&summary_content)
                .with_context(|| format!("Failed to parse JSON in {:?}", summary_path))?;

            // Convert nanoseconds to microseconds for better readability
            mean_values.push(summary.mean.floor() as u64);
        }

        data.insert(type_name, mean_values);
    }

    Ok(PlotData {
        types,
        executions,
        data,
    })
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
    width=12cm,
    height=8cm,
    xlabel={Executions},
    ylabel={Mean Time},
    legend style={at={(0.5,-0.2)}, anchor=north, legend columns=2},
    grid=major,
    grid style={dashed, gray!30},
    ybar interval=0.7,
    "#,
    );

    latex.push_str(format!("xtick={{1,2,...,{}}},\n", plot_data.executions.len()).as_str());

    latex.push_str("\txticklabels={\n");
    // Add execution names as x tick labels
    let labels = plot_data
        .executions
        .iter()
        .map(|e| format!("\t\t\\text{{{}}}", e.replace("_", "\\_")))
        .collect::<Vec<String>>()
        .join(",\n");
    latex.push_str(&labels);
    latex.push_str(
        r#"
    },
    x tick label style={rotate=45, anchor=east}
]
"#,
    );

    // Add plot for each type
    for (i, (type_name, values)) in plot_data.data.iter().enumerate() {
        latex.push_str("\\addplot coordinates {\n");

        for (x, y) in values.iter().enumerate() {
            latex.push_str(&format!("({},{})\n", x + 1, y));
        }

        latex.push_str(&format!("}};\n\\addlegendentry{{{}}};\n\n", type_name));
    }

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
    o.push("polybench.tex");

    println!("Collecting data from: {}", data_dir.display());
    let plot_data = collect_data(data_dir)?;

    println!("Found {} types:", plot_data.types.len());
    for type_name in &plot_data.types {
        println!("  - {}", type_name);
    }

    println!("Found {} executions:", plot_data.executions.len());
    for execution in &plot_data.executions {
        println!("  - {}", execution);
    }

    println!("Generating LaTeX plot...");
    generate_latex_plot(&plot_data, &o)
}
