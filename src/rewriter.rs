use crate::reader::ByteReader;
use crate::settings::Settings;
use crate::sink::RelaySink;
#[cfg(feature = "hyper")]
use crate::stream::ByteStream;
use crate::ByteQueue;
use atomic_waker::AtomicWaker;
use futures_core::ready;
use pin_project_lite::pin_project;
use std::io::{Error, ErrorKind};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{
    fmt::Debug,
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

pin_project! {
    #[derive(Debug)]
    pub struct Rewriter<'a, S>
    where
        S:AsyncRead
    {
        #[pin] source: S,
        rewriter: Option<lol_html::HtmlRewriter<'a, RelaySink >>,
        buffer: Vec<u8>,
        waker: Arc<AtomicWaker>,
        done: Arc<AtomicBool>,
        queue: Arc<ByteQueue>,
    }
}

impl<'a, S> Rewriter<'a, S>
where
    S: AsyncRead,
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
            buffer: vec![0; settings.buffer_size],
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


impl<S> Future for Rewriter<'_, S>
where
    S: AsyncRead,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let mut buffer = ReadBuf::new(this.buffer);
        let mut last_buffer_index = 0;
        loop {
            match this.rewriter {
                Some(rewriter) => match ready!(this.source.as_mut().poll_read(cx, &mut buffer)) {
                    Ok(()) => {
                        let source_is_exhausted = last_buffer_index == buffer.filled().len();
                        if source_is_exhausted {
                            // Best effort trying to immediately release rewriter resources
                            if let Some(rewriter) = this.rewriter.take() {
                                rewriter.end().unwrap_or_else(|err|
                                    eprintln!("unable to release html rewriter resources: {:?}", err)
                                )
                            }
                            return Poll::Ready(Ok(()));
                        } else {
                            let data = &buffer.filled()[last_buffer_index..];
                            if let Err(err) = rewriter.write(data) {
                                return Poll::Ready(Err(Error::new(ErrorKind::Other, err)));
                            }
                            last_buffer_index = buffer.filled().len();
                        }
                    }
                    Err(err) => {
                        return Poll::Ready(Err(Error::new(ErrorKind::Other, err)))
                    }
                },
                None => return Poll::Ready(Ok(())),
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

    #[tokio::test]
    async fn rewrite_html() {
        let expected = "<h1>Succeeded</h1>";
        let source = tokio_test::io::Builder::new()
            .read(b"<h1>Test</h1>")
            .build();

        let mut settings = Settings::new();
        settings.element_content_handlers = vec![element!("h1", |el| {
                el.replace(expected, ContentType::Html);
                Ok(())
        })];
        settings.buffer_size = 1024;

        let writer = Rewriter::new(source, settings);
        let mut reader = writer.output_reader();
        writer.await.unwrap();

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, expected);
    }
}
