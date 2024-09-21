use crate::context::Context;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::Poll;
use std::{io, task};
use tokio::io::{AsyncRead, ReadBuf};

pub struct ByteReader {
    context: Context,
}

impl ByteReader {
    pub fn new(context: Context) -> Self {
        Self { context }
    }
}

impl AsyncRead for ByteReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut task::Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        if !self.context.queue().is_empty() {
            if let Some(bytes) = self.context.queue().pop() {
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            } else {
                let err = io::Error::new(ErrorKind::Other, "data queue is empty");
                Poll::Ready(Err(err))
            }
        } else if self.context.is_done() {
            Poll::Ready(Ok(()))
        } else {
            self.context.register_waker(cx.waker());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::context::Context;
    use crate::reader::ByteReader;
    use crate::ByteQueue;
    use atomic_waker::AtomicWaker;
    use bytes::Bytes;
    use std::sync::atomic::AtomicBool;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn async_read_impl() {
        let input = "ABC";

        let queue = ByteQueue::new();
        queue.push(Bytes::from(input));

        let mut reader = ByteReader::new(Context::new(
            queue,
            AtomicWaker::default(),
            AtomicBool::new(true),
        ));

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, input);
    }
}