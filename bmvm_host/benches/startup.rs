use bmvm_host::{Buffer, RuntimeBuilder};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;

const NOOP: &str = "../bench/binaries/bmvm-guest-noop";

pub fn bmvm_setup_noop(c: &mut Criterion) {
    // Store path as String for reuse
    let buf = Buffer::new(NOOP).unwrap();
    let mut group = c.benchmark_group("bmvm-setup-noop");
    group.measurement_time(Duration::from_secs(30));

    group.bench_function("noop", |b| {
        b.iter(|| {
            // Build a fresh runtime for each iteration
            let runtime = RuntimeBuilder::new()
                .with_buffer(black_box(&buf))
                .build()
                .unwrap();

            // Setup and return the result
            runtime.setup().unwrap()
        })
    });
}

criterion_group!(benches, bmvm_setup_noop);
criterion_main!(benches);
