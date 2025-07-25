//! An adaptor that chunks up completed futures in a stream and flushes them after a timeout or when the buffer is full.
//! It is based on the `Chunks` adaptor of [futures-util](https://github.com/rust-lang-nursery/futures-rs/blob/4613193023dd4071bbd32b666e3b85efede3a725/futures-util/src/stream/chunks.rs), to which we added a timeout.
//!
//! ## Usage
//!
//! Either as a standalone stream operator or directly as a combinator:
//!
//! ```rust
//! use std::time::Duration;
//! use futures::{stream, StreamExt};
//! use futures_batch::ChunksTimeoutStreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let results = stream::iter(0..10)
//!         .chunks_timeout(5, Duration::from_secs(10))
//!         .collect::<Vec<_>>();
//!
//!     assert_eq!(vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]], results.await);
//! }
//! ```
//!
//! The above code iterates over a stream and creates chunks of size 5 with a timeout of 10 seconds.
//!
//! ## Try Chunks Timeout
//!
//! For streams of `Result` values, you can use `try_chunks_timeout` to batch successful values and immediately propagate errors:
//!
//! ```rust
//! use std::time::Duration;
//! use futures::{stream, StreamExt};
//! use futures_batch::TryChunksTimeoutStreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let results = stream::iter((0..10).map(|i| if i == 5 { Err("error") } else { Ok(i) }))
//!         .try_chunks_timeout(3, Duration::from_secs(10))
//!         .collect::<Vec<_>>();
//!
//!     // Results in: [Ok([0, 1, 2]), Ok([3, 4]), Err("error"), Ok([6, 7, 8]), Ok([9])]
//! }
//! ```

#[cfg(test)]
#[macro_use]
extern crate doc_comment;

#[cfg(test)]
doctest!("../README.md");

use core::mem;
use core::pin::Pin;
use futures::Future;
use futures::StreamExt;
use futures::stream::{Fuse, FusedStream, Stream};
use futures::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

use futures_timer::Delay;
use std::time::Duration;

/// A Stream extension trait allowing you to call `chunks_timeout` on anything
/// which implements `Stream`.
pub trait ChunksTimeoutStreamExt: Stream {
    fn chunks_timeout(self, capacity: usize, duration: Duration) -> ChunksTimeout<Self>
    where
        Self: Sized,
    {
        ChunksTimeout::new(self, capacity, duration)
    }
}
impl<T: ?Sized> ChunksTimeoutStreamExt for T where T: Stream {}

/// A Try Stream extension trait allowing you to call `try_chunks_timeout` on anything
/// which implements `Stream<Item = Result<T, E>>`.
pub trait TryChunksTimeoutStreamExt<T, E>: Stream<Item = Result<T, E>> {
    fn try_chunks_timeout(self, capacity: usize, duration: Duration) -> TryChunksTimeout<Self, T, E>
    where
        Self: Sized,
    {
        TryChunksTimeout::new(self, capacity, duration)
    }
}
impl<St, T, E> TryChunksTimeoutStreamExt<T, E> for St where St: Stream<Item = Result<T, E>> {}

pin_project! {
    /// A Stream of chunks.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunksTimeout<St: Stream> {
        #[pin]
        stream: Fuse<St>,
        items: Vec<St::Item>,
        cap: usize,
        // https://github.com/rust-lang-nursery/futures-rs/issues/1475
        #[pin]
        clock: Option<Delay>,
        duration: Duration,
    }
}

pin_project! {
    /// A Stream of chunks for Result streams that batches Ok values and immediately propagates errors.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryChunksTimeout<St: Stream<Item = Result<T, E>>, T, E> {
        #[pin]
        stream: Fuse<St>,
        items: Vec<T>,
        cap: usize,
        #[pin]
        clock: Option<Delay>,
        duration: Duration,
        pending_error: Option<E>,
    }
}

impl<St: Stream> ChunksTimeout<St>
where
    St: Stream,
{
    pub fn new(stream: St, capacity: usize, duration: Duration) -> ChunksTimeout<St> {
        assert!(capacity > 0);

        ChunksTimeout {
            stream: stream.fuse(),
            items: Vec::with_capacity(capacity),
            cap: capacity,
            clock: None,
            duration,
        }
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        self.stream.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut()
    }

    /// Acquires a pinned mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.project().stream.get_pin_mut()
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St, T, E> TryChunksTimeout<St, T, E>
where
    St: Stream<Item = Result<T, E>>,
{
    pub fn new(stream: St, capacity: usize, duration: Duration) -> TryChunksTimeout<St, T, E> {
        assert!(capacity > 0);

        TryChunksTimeout {
            stream: stream.fuse(),
            items: Vec::with_capacity(capacity),
            cap: capacity,
            clock: None,
            duration,
            pending_error: None,
        }
    }

    pub fn get_ref(&self) -> &St {
        self.stream.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut St {
        self.stream.get_mut()
    }

    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut St> {
        self.project().stream.get_pin_mut()
    }

    pub fn into_inner(self) -> St {
        self.stream.into_inner()
    }
}

impl<St: Stream> Stream for ChunksTimeout<St> {
    type Item = Vec<St::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(item) => match item {
                    // Push the item into the buffer and check whether it is full.
                    // If so, replace our buffer with a new and empty one and return
                    // the full one.
                    Some(item) => {
                        if this.items.is_empty() {
                            this.clock.as_mut().set(Some(Delay::new(*this.duration)));
                        }
                        this.items.push(item);
                        if this.items.len() >= *this.cap {
                            this.clock.as_mut().set(None);
                            let cap = *this.cap;
                            return Poll::Ready(Some(mem::replace(
                                this.items,
                                Vec::with_capacity(cap),
                            )));
                        } else {
                            // Continue the loop
                            continue;
                        }
                    }

                    // Since the underlying stream ran out of values, return what we
                    // have buffered, if we have anything.
                    None => {
                        let last = if this.items.is_empty() {
                            None
                        } else {
                            Some(mem::take(this.items))
                        };

                        return Poll::Ready(last);
                    }
                },
                // Don't return here, as we need to need check the clock.
                Poll::Pending => {}
            }

            match this.clock.as_mut().as_pin_mut().map(|clock| clock.poll(cx)) {
                Some(Poll::Ready(())) => {
                    this.clock.as_mut().set(None);
                    let cap = *this.cap;
                    return Poll::Ready(Some(mem::replace(this.items, Vec::with_capacity(cap))));
                }
                Some(Poll::Pending) => {}
                None => {
                    debug_assert!(
                        this.items.is_empty(),
                        "Inner buffer is empty, but clock is available."
                    );
                }
            }

            return Poll::Pending;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunk_len = if self.items.is_empty() { 0 } else { 1 };
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(chunk_len);
        let upper = match upper {
            Some(x) => x.checked_add(chunk_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St: FusedStream> FusedStream for ChunksTimeout<St> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() & self.items.is_empty()
    }
}

impl<St, T, E> Stream for TryChunksTimeout<St, T, E>
where
    St: Stream<Item = Result<T, E>>,
{
    type Item = Result<Vec<T>, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // First, check if we have a pending error to return
        if let Some(error) = this.pending_error.take() {
            return Poll::Ready(Some(Err(error)));
        }

        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(item) => match item {
                    Some(result) => match result {
                        Ok(item) => {
                            if this.items.is_empty() {
                                this.clock.as_mut().set(Some(Delay::new(*this.duration)));
                            }
                            this.items.push(item);
                            if this.items.len() >= *this.cap {
                                this.clock.as_mut().set(None);
                                let cap = *this.cap;
                                return Poll::Ready(Some(Ok(mem::replace(
                                    this.items,
                                    Vec::with_capacity(cap),
                                ))));
                            } else {
                                continue;
                            }
                        }
                        Err(e) => {
                            this.clock.as_mut().set(None);
                            if !this.items.is_empty() {
                                // Store the error for the next poll and return buffered items first
                                *this.pending_error = Some(e);
                                let cap = *this.cap;
                                let buffered_items =
                                    mem::replace(this.items, Vec::with_capacity(cap));
                                return Poll::Ready(Some(Ok(buffered_items)));
                            } else {
                                return Poll::Ready(Some(Err(e)));
                            }
                        }
                    },

                    None => {
                        let last = if this.items.is_empty() {
                            None
                        } else {
                            Some(Ok(mem::take(this.items)))
                        };

                        return Poll::Ready(last);
                    }
                },
                Poll::Pending => {}
            }

            match this.clock.as_mut().as_pin_mut().map(|clock| clock.poll(cx)) {
                Some(Poll::Ready(())) => {
                    this.clock.as_mut().set(None);
                    let cap = *this.cap;
                    return Poll::Ready(Some(Ok(mem::replace(
                        this.items,
                        Vec::with_capacity(cap),
                    ))));
                }
                Some(Poll::Pending) => {}
                None => {
                    debug_assert!(
                        this.items.is_empty(),
                        "Inner buffer is empty, but clock is available."
                    );
                }
            }

            return Poll::Pending;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let chunk_len = if self.items.is_empty() { 0 } else { 1 };
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(chunk_len);
        let upper = match upper {
            Some(x) => x.checked_add(chunk_len),
            None => None,
        };
        (lower, upper)
    }
}

impl<St, T, E> FusedStream for TryChunksTimeout<St, T, E>
where
    St: Stream<Item = Result<T, E>> + FusedStream,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated() & self.items.is_empty() & self.pending_error.is_none()
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for ChunksTimeout<S>
where
    S: Stream + Sink<Item>,
{
    type Error = S::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.project().stream.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_close(cx)
    }
}

#[cfg(feature = "sink")]
impl<S, Item, T, E> Sink<Item> for TryChunksTimeout<S, T, E>
where
    S: Stream<Item = Result<T, E>> + Sink<Item>,
{
    type Error = S::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.project().stream.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().stream.poll_close(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt, stream};
    use std::iter;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn messages_pass_through() {
        let results = stream::iter(iter::once(5))
            .chunks_timeout(5, Duration::from_secs(1))
            .collect::<Vec<_>>();
        assert_eq!(vec![vec![5]], results.await);
    }

    #[tokio::test]
    async fn message_chunks() {
        let stream = stream::iter(0..10);

        let chunk_stream = ChunksTimeout::new(stream, 5, Duration::from_secs(1));
        assert_eq!(
            vec![vec![0, 1, 2, 3, 4], vec![5, 6, 7, 8, 9]],
            chunk_stream.collect::<Vec<_>>().await
        );
    }

    #[tokio::test]
    async fn message_early_exit() {
        let iter = vec![1, 2, 3, 4].into_iter();
        let stream = stream::iter(iter);

        let chunk_stream = ChunksTimeout::new(stream, 5, Duration::from_secs(1));
        assert_eq!(
            vec![vec![1, 2, 3, 4]],
            chunk_stream.collect::<Vec<_>>().await
        );
    }

    #[tokio::test]
    async fn message_timeout() {
        let iter = vec![1, 2, 3, 4].into_iter();
        let stream0 = stream::iter(iter);

        let iter = vec![5].into_iter();
        let stream1 = stream::iter(iter)
            .then(move |n| Delay::new(Duration::from_millis(300)).map(move |_| n));

        let iter = vec![6, 7, 8].into_iter();
        let stream2 = stream::iter(iter);

        let stream = stream0.chain(stream1).chain(stream2);
        let chunk_stream = ChunksTimeout::new(stream, 5, Duration::from_millis(100));

        let now = Instant::now();
        let min_times = [Duration::from_millis(80), Duration::from_millis(150)];
        let max_times = [Duration::from_millis(350), Duration::from_millis(500)];
        let expected = vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]];
        let mut i = 0;

        let results = chunk_stream
            .map(move |s| {
                let now2 = Instant::now();
                println!("{}: {:?} {:?}", i, now2 - now, s);
                assert!((now2 - now) < max_times[i]);
                assert!((now2 - now) > min_times[i]);
                i += 1;
                s
            })
            .collect::<Vec<_>>();

        assert_eq!(results.await, expected);
    }

    #[tokio::test]
    async fn try_messages_pass_through() {
        let results = stream::iter(iter::once(Ok::<i32, &str>(5)))
            .try_chunks_timeout(5, Duration::from_secs(1))
            .collect::<Vec<_>>();
        assert_eq!(vec![Ok(vec![5])], results.await);
    }

    #[tokio::test]
    async fn try_message_chunks() {
        let stream = stream::iter((0..10).map(Ok::<i32, &str>));

        let chunk_stream = TryChunksTimeout::new(stream, 5, Duration::from_secs(1));
        assert_eq!(
            vec![Ok(vec![0, 1, 2, 3, 4]), Ok(vec![5, 6, 7, 8, 9])],
            chunk_stream.collect::<Vec<_>>().await
        );
    }

    #[tokio::test]
    async fn try_message_early_exit() {
        let iter = vec![1, 2, 3, 4].into_iter().map(Ok::<i32, &str>);
        let stream = stream::iter(iter);

        let chunk_stream = TryChunksTimeout::new(stream, 5, Duration::from_secs(1));
        assert_eq!(
            vec![Ok(vec![1, 2, 3, 4])],
            chunk_stream.collect::<Vec<_>>().await
        );
    }

    #[tokio::test]
    async fn try_error_propagation() {
        let iter = vec![Ok(1), Ok(2), Err("error"), Ok(4)].into_iter();
        let stream = stream::iter(iter);

        let chunk_stream = TryChunksTimeout::new(stream, 5, Duration::from_secs(1));
        let results = chunk_stream.collect::<Vec<_>>().await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Ok(vec![1, 2]));
        assert_eq!(results[1], Err("error"));
        assert_eq!(results[2], Ok(vec![4]));
    }

    #[tokio::test]
    async fn try_error_immediate_propagation() {
        let iter = vec![Err("immediate_error"), Ok(1), Ok(2)].into_iter();
        let stream = stream::iter(iter);

        let chunk_stream = TryChunksTimeout::new(stream, 5, Duration::from_secs(1));
        let results = chunk_stream.collect::<Vec<_>>().await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Err("immediate_error"));
        assert_eq!(results[1], Ok(vec![1, 2]));
    }

    #[tokio::test]
    async fn try_message_timeout() {
        let iter = vec![Ok::<i32, &str>(1), Ok(2), Ok(3), Ok(4)].into_iter();
        let stream0 = stream::iter(iter);

        let iter = vec![Ok::<i32, &str>(5)].into_iter();
        let stream1 = stream::iter(iter)
            .then(move |n| Delay::new(Duration::from_millis(300)).map(move |_| n));

        let iter = vec![Ok::<i32, &str>(6), Ok(7), Ok(8)].into_iter();
        let stream2 = stream::iter(iter);

        let stream = stream0.chain(stream1).chain(stream2);
        let chunk_stream = TryChunksTimeout::new(stream, 5, Duration::from_millis(100));

        let now = Instant::now();
        let min_times = [Duration::from_millis(80), Duration::from_millis(150)];
        let max_times = [Duration::from_millis(350), Duration::from_millis(500)];
        let expected = vec![Ok(vec![1, 2, 3, 4]), Ok(vec![5, 6, 7, 8])];
        let mut i = 0;

        let results = chunk_stream
            .map(move |s| {
                let now2 = Instant::now();
                println!("{}: {:?} {:?}", i, now2 - now, s);
                assert!((now2 - now) < max_times[i]);
                assert!((now2 - now) > min_times[i]);
                i += 1;
                s
            })
            .collect::<Vec<_>>();

        assert_eq!(results.await, expected);
    }
}
