use bmvm_host::ModuleBuilder;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

const NOOP: &str = "../bench/binaries/bmvm-guest-noop";

pub fn bmvm_setup_noop(c: &mut Criterion) {
    // Store path as String for reuse
    let path = PathBuf::from(NOOP);
    let mut group = c.benchmark_group("bmvm-setup-noop");
    group.measurement_time(Duration::from_secs(30));

    // let builder = ModuleBuilder::new().with_path(&path);

    group.bench_function("noop", |b| {
        b.iter(|| {
            let builder = ModuleBuilder::new().with_path(&path);
            // Build a fresh runtime for each iteration
            black_box(builder.build().unwrap());
        })
    });
}

criterion_group!(benches, bmvm_setup_noop);
criterion_main!(benches);
