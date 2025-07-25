# futures-batch

![Build status](https://github.com/mre/futures-batch/workflows/Rust/badge.svg)
[![Cargo](https://img.shields.io/crates/v/futures-batch.svg)](https://crates.io/crates/futures-batch)
[![Documentation](https://docs.rs/futures-batch/badge.svg)](https://docs.rs/futures-batch)

A stream adaptor that chunks up items with timeout support. Items are flushed when:
- The buffer reaches capacity **or**
- A timeout occurs

Based on the `Chunks` adaptor from [futures-util](https://github.com/rust-lang/futures-rs), with added timeout functionality.

> **Note:** Originally called `tokio-batch`, but renamed since it has no dependency on Tokio.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
futures-batch = "0.7"
```

Use as a stream combinator:

```rust
use std::time::Duration;
use futures::{stream, StreamExt};
use futures_batch::ChunksTimeoutStreamExt;

#[tokio::main]
async fn main() {
    let results = stream::iter(0..10)
        .chunks_timeout(5, Duration::from_secs(10))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]], results);
}
```

This creates chunks of up to 5 items with a 10-second timeout.

## Features

### `sink` (optional)

Enable `Sink` support for bidirectional streams:

```toml
[dependencies]
futures-batch = { version = "0.7", features = ["sink"] }
```

When enabled, `ChunksTimeout` implements `Sink` and forwards sink operations to the underlying stream.

## Performance

`futures-batch` has minimal overhead and is suitable for high-performance applications:

- Used for [batching syscalls](https://github.com/mre/futures-batch/issues/4) in production
- Built on [`futures-timer`](https://github.com/async-rs/futures-timer) with microsecond resolution
- Zero allocations for chunk creation (reuses capacity)

Benchmarks show consistent ~20ns per operation across different batch sizes.

## Credits

Thanks to [arielb1](https://github.com/arielb1), [alexcrichton](https://github.com/alexcrichton/), [doyoubi](https://github.com/doyoubi), [leshow](https://github.com/leshow), [spebern](https://github.com/spebern), and [wngr](https://github.com/wngr) for their contributions!
