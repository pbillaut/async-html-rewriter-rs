use crate::reader::ByteReader;
use crate::settings::Settings;
use crate::sink::RelaySink;
#[cfg(feature = "hyper")]
use crate::stream::ByteStream;
use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use futures_core::{ready, Stream};
use pin_project_lite::pin_project;
use std::{
    fmt::Debug,
    future::Future,
    io,
    pin::Pin,
    sync::atomic::AtomicBool,
    sync::Arc,
    task::{Context, Poll},
};

pin_project! {
    #[derive(Debug)]
    pub struct Rewriter<'a, S, I>
    where
        S: Stream<Item=I>,
        I: AsRef<[u8]>,
    {
        #[pin] source: S,
        rewriter: Option<lol_html::HtmlRewriter<'a, RelaySink >>,
        waker: Arc<AtomicWaker>,
        done: Arc<AtomicBool>,
        queue: Arc<ByteQueue>,
    }
}

impl<'a, S, I> Rewriter<'a, S, I>
where
    S: Stream<Item=I>,
    I: AsRef<[u8]>,
{
    pub fn new(source: S, settings: Settings<'a, '_>) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        let done = Arc::new(AtomicBool::default());
        let queue = Arc::new(ByteQueue::new());

        let sink = RelaySink::new(queue.clone(), waker.clone(), done.clone());
        Self {
            queue,
            waker,
            done,
            source,
            rewriter: Some(lol_html::HtmlRewriter::new(settings.into_inner(), sink)),
        }
    }

    pub fn with_queue(
        source: S,
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
            source,
            rewriter: Some(lol_html::HtmlRewriter::new(settings.into_inner(), sink)),
        }
    }

    pub fn output_reader(&self) -> ByteReader {
        ByteReader::new(self.queue.clone(), self.waker.clone(), self.done.clone())
    }

    #[cfg(feature = "hyper")]
    pub fn output_stream(&self) -> ByteStream {
        ByteStream::new(self.queue.clone(), self.waker.clone(), self.done.clone())
    }
}

impl<S, I> Future for Rewriter<'_, S, I>
where
    S: Stream<Item=I>,
    I: AsRef<[u8]>,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        match this.rewriter {
            None => {
                // This should be unreachable, since awaiting this future consumes this object
                let err = io::Error::new(io::ErrorKind::UnexpectedEof, "Writer has been closed");
                Poll::Ready(Err(err))
            }
            Some(rewriter) => loop {
                match ready!(this.source.as_mut().poll_next(cx)) {
                    Some(item) => {
                        if let Err(err) = rewriter.write(item.as_ref()) {
                            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, err)));
                        }
                    }
                    None => {
                        // Best effort trying to immediately release rewriter resources
                        if let Some(rewriter) = this.rewriter.take() {
                            // We're ignoring failed attempts
                            let _ = rewriter.end();
                        }
                        return Poll::Ready(Ok(()));
                    }
                }
            }
        }
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
        let source = StreamMockBuilder::new()
            .next(b"<h1>Test</h1>")
            .build();

        let mut settings = Settings::new();
        settings.element_content_handlers = vec![element!("h1", |el| {
                el.replace(expected, ContentType::Html);
                Ok(())
        })];

        let rewriter = Rewriter::new(source, settings);
        let mut reader = rewriter.output_reader();
        rewriter.await.unwrap();

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, expected);
    }
}
