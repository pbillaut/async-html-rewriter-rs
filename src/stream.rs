use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use bytes::Bytes;
use futures_core::stream::Stream;
use http_body::Frame;
use std::io::{Error, ErrorKind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

pub struct ByteStream {
    queue: Arc<ByteQueue>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
}

impl ByteStream {
    pub fn new(queue: Arc<ByteQueue>, waker: Arc<AtomicWaker>, done: Arc<AtomicBool>) -> Self {
        Self { queue, waker, done }
    }
}

impl Stream for ByteStream {
    type Item = Result<Frame<Bytes>, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.queue.is_empty() {
            if let Some(bytes) = self.queue.pop() {
                let frame = Frame::data(bytes);
                Poll::Ready(Some(Ok(frame)))
            } else {
                let err = Error::new(ErrorKind::Other, "data queue is empty");
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

impl AsyncRead for ByteStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<std::io::Result<()>> {
        if !self.queue.is_empty() {
            if let Some(bytes) = self.queue.pop() {
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            } else {
                let err = Error::new(ErrorKind::Other, "data queue is empty");
                Poll::Ready(Err(err))
            }
        } else if self.done.load(Ordering::SeqCst) {
            Poll::Ready(Ok(()))
        } else {
            self.waker.register(cx.waker());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio_stream::StreamExt;

    fn test_instance(input: &'static str) -> ByteStream {
        let queue = ByteQueue::new();
        queue.push(Bytes::from(input));

        ByteStream::new(
            Arc::new(queue),
            Arc::new(AtomicWaker::new()),
            Arc::new(AtomicBool::new(true)),
        )
    }

    #[tokio::test]
    async fn async_read_impl() {
        let input = "ABC";
        let mut reader = test_instance(input);

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, input);
    }

    #[tokio::test]
    async fn stream_impl() {
        let input = "ABC";
        let stream = test_instance(input);

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
