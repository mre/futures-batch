# futures-batch

![Build status](https://github.com/mre/futures-batch/workflows/Rust/badge.svg)
[![Cargo](https://img.shields.io/crates/v/futures-batch.svg)](
https://crates.io/crates/futures-batch)
[![Documentation](https://docs.rs/futures-batch/badge.svg)](
https://docs.rs/futures-batch)

An adaptor that chunks up completed futures in a stream and flushes them after a timeout or when the buffer is full.
It is based on the `Chunks` adaptor of [futures-util](https://github.com/rust-lang-nursery/futures-rs/blob/4613193023dd4071bbd32b666e3b85efede3a725/futures-util/src/stream/chunks.rs), to which we added a timeout.

(The project was initially called `tokio-batch`, but was renamed as it has no dependency on Tokio anymore.)

## Usage

Either as a standalone stream operator or directly as a combinator:

```rust
use std::time::Duration;
use futures::{stream, StreamExt};
use futures_batch::ChunksTimeoutStreamExt;

#[tokio::main]
async fn main() {
    let iter = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter();
    let results = stream::iter(iter)
        .chunks_timeout(5, Duration::new(10, 0))
        .collect::<Vec<_>>();

    assert_eq!(vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]], results.await);
}
```

The above code iterates over a stream and creates chunks of size 5 with a timeout of 10 seconds.  
*Note:* This is using the [`futures 0.3`](https://crates.io/crates/futures) crate.

## Performance

`futures-batch` imposes very low overhead on your application. For example, it [is even used to batch syscalls](https://github.com/mre/futures-batch/issues/4).  
Under the hood, we are using [`futures-timer`](https://github.com/async-rs/futures-timer), which allows for a microsecond timer resolution.
If you find a use-case which is not covered, don't be reluctant to open an issue.

## Credits

Thanks to [arielb1](https://github.com/arielb1), [alexcrichton](https://github.com/alexcrichton/), [doyoubi](https://github.com/doyoubi), [leshow](https://github.com/leshow), [spebern](https://github.com/spebern), and [wngr](https://github.com/wngr) for their contributions!
