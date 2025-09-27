import os
import json
import argparse

SUMMARY_FILE = "summary.json"

def get_mean_from_summary(path):
    try:
        with open(path, "r") as f:
            data = json.load(f)
        return data.get("mean", None)
    except Exception as e:
        print(f"Failed to read {path}: {e}")
        return None

def collect_means(base_path):
    means = {}
    if not os.path.exists(base_path):
        print(f"Path does not exist: {base_path}")
        return means
    for benchmark in os.listdir(base_path):
        summary_path = os.path.join(base_path, benchmark, SUMMARY_FILE)
        if os.path.exists(summary_path):
            mean = get_mean_from_summary(summary_path)
            if mean is not None:
                means[benchmark] = mean
    return means

def main():
    parser = argparse.ArgumentParser(description="Normalize benchmark means against reference.")
    parser.add_argument("--base-dir", type=str, required=True,
                        help="Base directory containing execution engine subdirectories")
    parser.add_argument("--engines", type=str, nargs="+", required=True,
                        help="List of engines to normalize (e.g. bmvm wasm)")
    parser.add_argument("--reference", type=str, required=True,
                        help="Reference version to normalize against (e.g. native)")
    parser.add_argument("--output-csv", type=str, default=None,
                        help="Optional path to save results as CSV")

    args = parser.parse_args()

    base_dir = args.base_dir
    engines = args.engines
    reference = args.reference

    # Collect reference means
    reference_path = os.path.join(base_dir, reference)
    reference_means = collect_means(reference_path)

    all_results = {}

    for engine in engines:
        engine_path = os.path.join(base_dir, engine)
        engine_means = collect_means(engine_path)
        normalized = {}

        for bench, mean in engine_means.items():
            ref_mean = reference_means.get(bench)
            if ref_mean:
                normalized_value = (mean / ref_mean) * 100
                normalized[bench] = normalized_value
            else:
                normalized[bench] = None  # No reference available

        all_results[engine] = normalized

    print(f"{all_results}")

    # Prepare output
    benchmarks = sorted(set(k for v in all_results.values() for k in v))
    header = f"{'Benchmark':<20}" + "".join([f"{ver:>12}" for ver in engines])
    print(header)
    print("-" * len(header))
    lines = []
    for bench in benchmarks:
        row = [bench.ljust(20)]
        for engine in engines:
            val = all_results[engine].get(bench)
            val_str = f"{val:.2f}" if val is not None else "N/A"
            row.append(val_str.rjust(12))
        print("".join(row))
        lines.append([bench] + [all_results[ver].get(bench, "N/A") for ver in engines])

    # Optional CSV output
    if args.output_csv:
        import csv
        with open(args.output_csv, "w", newline="") as f:
            writer = csv.writer(f)
            writer.writerow(["Benchmark"] + engines)
            for line in lines:
                row = [line[0]]
                for val in line[1:]:
                    row.append(f"{val:.2f}" if isinstance(val, (int, float)) else "N/A")
                writer.writerow(row)
        print(f"\nResults saved to {args.output_csv}")

if __name__ == "__main__":
    main()
