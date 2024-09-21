#![cfg(feature = "hyper")]
use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use bytes::Bytes;
use futures_core::{ready, stream::Stream};
use http_body::{Body, Frame};
use std::io;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

pub struct FrameStream {
    queue: Arc<ByteQueue>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
}

impl FrameStream {
    pub fn new(queue: Arc<ByteQueue>, waker: Arc<AtomicWaker>, done: Arc<AtomicBool>) -> Self {
        Self { queue, waker, done }
    }
}

impl Stream for FrameStream {
    type Item = io::Result<Frame<Bytes>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.queue.is_empty() {
            if let Some(bytes) = self.queue.pop() {
                let frame = Frame::data(bytes);
                Poll::Ready(Some(Ok(frame)))
            } else {
                let err = io::Error::new(ErrorKind::Other, "data queue is empty");
                Poll::Ready(Some(Err(err)))
            }
        } else if self.done.load(Ordering::SeqCst) {
            Poll::Ready(None)
        } else {
            self.waker.register(cx.waker());
            Poll::Pending
        }
    }
}

impl Body for FrameStream {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match ready!(self.poll_next(cx)) {
            None => Poll::Ready(None),
            Some(frame) => Poll::Ready(Some(frame)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrameStream;
    use crate::ByteQueue;
    use atomic_waker::AtomicWaker;
    use bytes::Bytes;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn stream_impl() {
        let input = "ABC";

        let queue = ByteQueue::new();
        queue.push(Bytes::from(input));

        let stream = FrameStream::new(
            Arc::new(queue),
            Arc::new(AtomicWaker::new()),
            Arc::new(AtomicBool::new(true)),
        );

        let output: Vec<String> = stream
            .map(|frame| frame
                .expect("Expected stream to contain valid frames")
                .into_data()
                .expect("Expected frames to hold valid data")
            )
            .map(|bytes|
                std::str::from_utf8(&bytes)
                    .expect("Expected frame data to be valid utf8")
                    .to_string()
            )
            .collect().await;

        assert_eq!(output.len(), 1);
        assert_eq!(output.first().unwrap(), input);
    }
}
