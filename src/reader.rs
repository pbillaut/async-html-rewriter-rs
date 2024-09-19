use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use std::io;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

pub struct ByteReader {
    queue: Arc<ByteQueue>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
}

impl ByteReader {
    pub fn new(queue: Arc<ByteQueue>, waker: Arc<AtomicWaker>, done: Arc<AtomicBool>) -> Self {
        Self { queue, waker, done }
    }
}

impl AsyncRead for ByteReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        if !self.queue.is_empty() {
            if let Some(bytes) = self.queue.pop() {
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            } else {
                let err = io::Error::new(ErrorKind::Other, "data queue is empty");
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
    use bytes::Bytes;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn async_read_impl() {
        let input = "ABC";

        let queue = ByteQueue::new();
        queue.push(Bytes::from(input));

        let mut reader = ByteReader::new(
            Arc::new(queue),
            Arc::new(AtomicWaker::new()),
            Arc::new(AtomicBool::new(true)),
        );

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, input);
    }
}