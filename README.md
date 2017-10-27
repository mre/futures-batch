# tokio-batch

An adaptor that chunks up elements and flushes them after a timeout or when the buffer is full.

## Description

An adaptor that chunks up elements in a vector.

This adaptor will buffer up a list of items in the stream and pass on the
vector used for buffering when a specified capacity has been reached
or a predefined timeout was triggered.

## Usage

```rust
extern crate tokio_core;
extern crate futures;
extern crate tokio_batch;

use tokio_core::reactor::Core;
use futures::{stream, Stream};
use std::io;
use std::time::Duration;
use tokio_batch::*;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let iter = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9].into_iter();
    let stream = stream::iter_ok::<_, io::Error>(iter);

    let chunk_stream = Chunks::new(stream, handle, 5, Duration::new(10, 0));

    let v = chunk_stream.collect();
    let result = core.run(v).unwrap();
    assert_eq!(vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]], result);
}
```

## Credits

This was taken and adjusted from
https://github.com/alexcrichton/futures-rs/blob/master/src/stream/chunks.rs
and moved into a separate crate for usability.

Thanks to [@arielb1](https://github.com/arielb1) and [@alexcrichton](https://github.com/alexcrichton/) for their support!