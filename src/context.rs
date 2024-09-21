use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::Waker;

#[derive(Debug, Default, Clone)]
pub struct Context {
    queue: Arc<ByteQueue>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
}

impl Context {
    pub fn new(queue: ByteQueue, waker: AtomicWaker, done: AtomicBool) -> Self {
        Self { queue: Arc::new(queue), waker: Arc::new(waker), done: Arc::new(done) }
    }

    pub fn queue(&self) -> &Arc<ByteQueue> {
        &self.queue
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    pub fn set_done(&mut self) {
        self.done.store(true, Ordering::SeqCst)
    }

    pub fn register_waker(&self, waker: &Waker) {
        self.waker.register(waker)
    }

    pub fn wake(&self) {
        self.waker.wake()
    }
}
