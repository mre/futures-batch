extern crate futures;
extern crate tokio_core;

use std::io;
use std::time::Duration;
use std::mem;
use std::prelude::v1::*;

use tokio_core::reactor::{Timeout, Handle};
use futures::{Async, Future, Poll};
use futures::stream::{Stream, Fuse};

/// An adaptor that chunks up elements in a vector.
///
/// This adaptor will buffer up a list of items in the stream and pass on the
/// vector used for buffering when a specified capacity has been reached
/// or a predefined timeout was triggered.
///
/// This was taken and adjusted from
/// https://github.com/alexcrichton/futures-rs/blob/master/src/stream/chunks.rs
/// and moved into a separate crate for usability.
#[must_use = "streams do nothing unless polled"]
pub struct Chunks<S>
where
    S: Stream,
{
    handle: Handle,
    clock: Option<Timeout>,
    duration: Duration,
    items: Vec<S::Item>,
    err: Option<S::Error>,
    stream: Fuse<S>,
}

impl<S> Chunks<S>
where
    S: Stream,
{
    pub fn new(s: S, handle: Handle, capacity: usize, duration: Duration) -> Chunks<S> {
        assert!(capacity > 0);

        Chunks {
            handle: handle,
            clock: None,
            duration: duration,
            items: Vec::with_capacity(capacity),
            err: None,
            stream: s.fuse(),
        }
    }

    fn take(&mut self) -> Vec<S::Item> {
        let cap = self.items.capacity();
        mem::replace(&mut self.items, Vec::with_capacity(cap))
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &S {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut S {
        self.stream.get_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }

    fn flush(&mut self) -> Poll<Option<Vec<S::Item>>, S::Error> {
        self.clock = None;
        return Ok(Some(self.take()).into());
    }
}

impl<S> Stream for Chunks<S>
where
    S: Stream,
    <S as Stream>::Error: From<io::Error>,
{
    type Item = Vec<<S as Stream>::Item>;
    type Error = <S as Stream>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Some(err) = self.err.take() {
            return Err(err);
        }

        let cap = self.items.capacity();
        loop {
            match self.stream.poll() {
                Ok(Async::NotReady) => {}

                // Push the item into the buffer and check whether it is full.
                // If so, replace our buffer with a new and empty one and return
                // the full one.
                Ok(Async::Ready(Some(item))) => {
                    if self.items.is_empty() {
                        self.clock = Some(Timeout::new(self.duration, &self.handle).unwrap());
                    }
                    self.items.push(item);
                    if self.items.len() >= cap {
                        return self.flush();
                    } else {
                        continue;
                    }
                }

                // Since the underlying stream ran out of values, return what we
                // have buffered, if we have anything.
                Ok(Async::Ready(None)) => {
                    return if self.items.len() > 0 {
                        let full_buf = mem::replace(&mut self.items, Vec::new());
                        Ok(Some(full_buf).into())
                    } else {
                        Ok(Async::Ready(None))
                    }
                }

                // If we've got buffered items be sure to return them first,
                // we'll defer our error for later.
                Err(e) => {
                    if self.items.len() == 0 {
                        return Err(e);
                    } else {
                        self.err = Some(e);
                        return self.flush();
                    }
                }
            }

            match self.clock.poll() {
                Ok(Async::Ready(Some(()))) => {
                     return Ok(Some(self.take()).into());
                }
                Ok(Async::Ready(None)) => {
                    assert!(self.items.is_empty(), "no clock but there are items");
                }
                Ok(Async::NotReady) => {}
                Err(e) => {
                    if self.items.len() == 0 {
                        return Err(From::from(e));
                    } else {
                        self.err = Some(From::from(e));
                        return self.flush();
                    }
                }
            }

            return Ok(Async::NotReady);
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_core::reactor::Core;
    use futures::{stream, Stream};
    use std::iter;
    use std::time::Duration;
    use super::*;

    #[test]
    fn it_works() {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let iter = iter::once(5);
        let stream = stream::iter_ok::<_, io::Error>(iter);

        let chunk_stream = Chunks::new(stream, handle, 10, Duration::new(10, 0));

        let v = chunk_stream.collect();
        let result = core.run(v).unwrap();
        assert_eq!(vec![vec![5]], result);
    }
}
