#![deny(clippy::cargo_common_metadata)]
#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::missing_assert_message)]

pub mod sink;
pub mod stream;
pub mod rewriter;
pub mod settings;

use bytes::Bytes;
use crossbeam_queue::SegQueue;

pub type ByteQueue = SegQueue<Bytes>;
