use crate::reader::ByteReader;
use crate::settings::Settings;
use crate::sink::RelaySink;
#[cfg(feature = "hyper")]
use crate::stream::FrameStream;
use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use futures_core::Stream;
use std::{
    fmt::Debug,
    sync::atomic::AtomicBool,
    sync::Arc,
};
use thiserror::Error;
use tokio_stream::StreamExt;

#[derive(Error, Debug)]
pub enum RewriterError {
    #[error("rewriting error: {0}")]
    RewritingError(#[from] lol_html::errors::RewritingError),

    #[cfg(feature = "hyper")]
    #[error("hyper error: {0}")]
    HyperError(#[from] hyper::Error),
}

pub type RewriterResult<T> = Result<T, RewriterError>;

#[derive(Debug)]
pub struct Rewriter<'a> {
    rewriter: Option<lol_html::HtmlRewriter<'a, RelaySink>>,
    waker: Arc<AtomicWaker>,
    done: Arc<AtomicBool>,
    queue: Arc<ByteQueue>,
}

impl<'a> Rewriter<'a> {
    pub fn new(settings: Settings<'a, '_>) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        let done = Arc::new(AtomicBool::default());
        let queue = Arc::new(ByteQueue::new());

        let sink = RelaySink::new(queue.clone(), waker.clone(), done.clone());
        Self {
            queue,
            waker,
            done,
            rewriter: Some(lol_html::HtmlRewriter::new(settings.into_inner(), sink)),
        }
    }

    pub fn with_queue(
        settings: Settings<'a, '_>,
        queue: Arc<ByteQueue>,
        waker: Arc<AtomicWaker>,
        done: Arc<AtomicBool>,
    ) -> Self {
        let sink = RelaySink::new(queue.clone(), waker.clone(), done.clone());
        Self {
            queue,
            waker,
            done,
            rewriter: Some(lol_html::HtmlRewriter::new(settings.into_inner(), sink)),
        }
    }

    pub fn output_reader(&self) -> ByteReader {
        ByteReader::new(self.queue.clone(), self.waker.clone(), self.done.clone())
    }

    pub async fn rewrite<S, I>(mut self, stream: &mut S) -> RewriterResult<()>
    where
        S: Stream<Item=I> + Unpin,
        I: AsRef<[u8]>,
    {
        match &mut self.rewriter {
            None => unreachable!("The writer should only ever be None when drop has been called"),
            Some(rewriter) => {
                while let Some(item) = stream.next().await {
                    rewriter.write(item.as_ref())?
                }
            }
        }
        Ok(())
    }
}

impl<'a> Drop for Rewriter<'a> {
    fn drop(&mut self) {
        // Best effort to close the inner rewriter
        if let Some(rewriter) = self.rewriter.take() {
            let _ = rewriter.end();
        }
    }
}

#[cfg(feature = "hyper")]
impl<'a> Rewriter<'a> {
    pub fn output_stream(&self) -> FrameStream {
        FrameStream::new(self.queue.clone(), self.waker.clone(), self.done.clone())
    }

    pub async fn rewrite_body<S, I>(mut self, stream: &mut S) -> RewriterResult<()>
    where
        S: Stream<Item=hyper::Result<I>> + Unpin,
        I: AsRef<[u8]>,
    {
        match &mut self.rewriter {
            None => unreachable!("The writer should only ever be None when drop has been called"),
            Some(rewriter) => {
                while let Some(item) = stream.next().await {
                    rewriter.write(item?.as_ref())?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::rewriter::Rewriter;
    use crate::settings::Settings;
    use lol_html::element;
    use lol_html::html_content::ContentType;
    use tokio::io::AsyncReadExt;
    use tokio_test::stream_mock::StreamMockBuilder;

    #[tokio::test]
    async fn rewrite_html() {
        let expected = "<h1>Succeeded</h1>";
        let mut source = StreamMockBuilder::new()
            .next(b"<h1>Test</h1>")
            .build();

        let mut settings = Settings::new();
        settings.element_content_handlers = vec![element!("h1", |el| {
                el.replace(expected, ContentType::Html);
                Ok(())
        })];

        let rewriter = Rewriter::new(settings);
        let mut reader = rewriter.output_reader();
        let result = rewriter.rewrite(&mut source).await;
        assert!(result.is_ok(), "Expected rewriting to succeed: {:?}", result);

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, expected);
    }
}
