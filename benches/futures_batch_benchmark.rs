use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use futures::stream::{self, Stream};
use futures_batch::ChunksTimeoutStreamExt;
use std::time::Duration;
use tokio::runtime::Runtime;

async fn batch(stream: impl Stream<Item = i32> + Unpin) {
    let _ = stream.chunks_timeout(5, Duration::new(10, 0));
}

fn criterion_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("futures_batch");
    for &size in &[1000, 10_000, 100_000, 1_000_000, 10_000_000] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&rt).iter(|| batch(stream::iter(0..size)));
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
