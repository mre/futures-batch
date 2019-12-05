# futures-batch

![Build status](https://github.com/mre/futures-batch/workflows/Rust/badge.svg)

An adaptor that chunks up completed futures in a stream and flushes them after a timeout or when the buffer is full.
(The project was initially called `tokio-batch`, but was renamed as it has no dependency on Tokio anymore.)

## Description

An adaptor that chunks up completed futures in a stream.

This adaptor will buffer up a list of items in a stream and pass on the
collection used for buffering when a specified capacity has been reached
or a predefined timeout was triggered.

## Usage

Either as a standalone stream operator or directly as a combinator:

```rust
use futures::future;
use futures::stream;
use futures::{FutureExt, StreamExt, TryFutureExt};
use std::time::Duration;
use futures_batch::ChunksTimeoutStreamExt;

#[tokio::main]
async fn main() {
    let iter = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter();
    let v = stream::iter(iter)
        .chunks_timeout(5, Duration::new(10, 0))
        .collect::<Vec<_>>();

    assert_eq!(vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]], v.await);
}
```

_Note: This is using the [`futures-preview`](https://crates.io/crates/futures-preview) crate.
Check [this blog post](https://rust-lang-nursery.github.io/futures-rs/blog/2019/04/18/compatibility-layer.html) about the futures-rs compability layer._

## Performance

`futures-batch` imposes very low overhead on your application. For example, it [is even used to batch syscalls](https://github.com/mre/futures-batch/issues/4).  
Under the hood, we are using [`futures-timer`](https://github.com/async-rs/futures-timer), which allows for microsecond timer resolution.
If you find a use-case which is not covered, don't be reluctant to open an issue.

## Credits

This was taken and adjusted from [futures-util](https://github.com/rust-lang-nursery/futures-rs/blob/4613193023dd4071bbd32b666e3b85efede3a725/futures-util/src/stream/chunks.rs) and moved into a separate crate for reusability.
Since then it has been modified to support higher-resolution timers.

Thanks to [arielb1](https://github.com/arielb1), [alexcrichton](https://github.com/alexcrichton/), [doyoubi](https://github.com/doyoubi), [spebern](https://github.com/spebern), [wngr](https://github.com/wngr) for their contributions!
