use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use bytes::Bytes;
use lol_html::OutputSink;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

#[derive(Debug)]
pub struct RelaySink {
    queue: Arc<ByteQueue>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
}

impl RelaySink {
    pub fn new(queue: Arc<ByteQueue>, waker: Arc<AtomicWaker>, done: Arc<AtomicBool>) -> Self {
        Self { queue, waker, done }
    }
}

impl OutputSink for RelaySink {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            self.done.store(true, Ordering::SeqCst);
        } else {
            let bytes = Bytes::copy_from_slice(chunk);
            self.queue.push(bytes);
        }
        self.waker.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::RelaySink;
    use crate::ByteQueue;
    use atomic_waker::AtomicWaker;
    use lol_html::OutputSink;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn handle_chunk() {
        let input: Vec<&[u8]> = vec![b"chunk[0]", b"chunk[1]", b"", b"<invalid>"];
        let expected: Vec<u8> = b"chunk[0]chunk[1]".into();

        let waker = Arc::new(AtomicWaker::new());
        let done = Arc::new(AtomicBool::default());
        let queue = Arc::new(ByteQueue::default());

        let mut sink = RelaySink::new(queue.clone(), waker.clone(), done.clone());

        for chunk in input {
            sink.handle_chunk(chunk);
            if done.load(Ordering::SeqCst) {
                break;
            }
        }

        let mut output = Vec::new();
        while let Some(bytes) = queue.pop() {
            output.extend_from_slice(&bytes);
        }

        assert_eq!(output, expected);
    }
}