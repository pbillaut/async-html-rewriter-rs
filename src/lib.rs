#![deny(clippy::cargo_common_metadata)]
#![deny(clippy::panic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::missing_assert_message)]

pub mod reader;
pub mod rewriter;
pub mod settings;
pub mod sink;
pub mod stream;

use bytes::Bytes;
use crossbeam_queue::SegQueue;

pub type ByteQueue = SegQueue<Bytes>;
