use crate::context::Context;
use crate::reader::ByteReader;
use crate::settings::Settings;
use crate::sink::RelaySink;
#[cfg(feature = "hyper")]
use crate::stream::FrameStream;
use bytes::BytesMut;
use futures_core::Stream;
use std::fmt::Debug;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_stream::StreamExt;

#[derive(Error, Debug)]
pub enum RewriterError {
    #[error(transparent)]
    Rewriter(#[from] lol_html::errors::RewritingError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "hyper")]
    #[error(transparent)]
    Hyper(#[from] hyper::Error),

}

pub type RewriterResult<T> = Result<T, RewriterError>;

#[derive(Debug)]
pub struct Rewriter<'a> {
    context: Context,
    rewriter: lol_html::HtmlRewriter<'a, RelaySink>,
    reader_buf_size: usize,
}

impl<'a> Rewriter<'a> {
    pub fn new(settings: Settings<'a, '_>) -> Self {
        Self::with_context(settings, Context::default())
    }

    pub fn with_context(
        settings: Settings<'a, '_>,
        context: Context,
    ) -> Self {
        let sink = RelaySink::new(context.clone());
        Self {
            context,
            reader_buf_size: settings.reader_buf_size,
            rewriter: lol_html::HtmlRewriter::new(settings.into_inner(), sink),
        }
    }

    pub fn output_reader(&self) -> ByteReader {
        ByteReader::new(self.context.clone())
    }

    pub async fn rewrite_stream<S, I>(mut self, stream: &mut S) -> RewriterResult<()>
    where
        S: Stream<Item=I> + Unpin,
        I: AsRef<[u8]>,
    {
        while let Some(item) = stream.next().await {
            self.rewriter.write(item.as_ref())?;
        }
        self.rewriter.end()?;
        Ok(())
    }

    pub async fn rewrite_reader<R>(mut self, reader: &mut R) -> RewriterResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let mut buffer = BytesMut::zeroed(self.reader_buf_size);
        loop {
            match reader.read(&mut buffer[..]).await? {
                0 => break,
                n => self.rewriter.write(&buffer[..n])?,
            }
        }
        self.rewriter.end()?;
        Ok(())
    }
}

#[cfg(feature = "hyper")]
impl<'a> Rewriter<'a> {
    pub fn output_stream(&self) -> FrameStream {
        FrameStream::new(self.context.clone())
    }

    pub async fn rewrite_body<S, I>(mut self, stream: &mut S) -> RewriterResult<()>
    where
        S: Stream<Item=hyper::Result<I>> + Unpin,
        I: AsRef<[u8]>,
    {
        while let Some(item) = stream.next().await {
            self.rewriter.write(item?.as_ref())?;
        }
        self.rewriter.end()?;
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
    async fn rewrite_stream() {
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
        let result = rewriter.rewrite_stream(&mut source).await;
        assert!(result.is_ok(), "Expected rewriting to succeed: {:?}", result);

        let mut output = String::new();
        let bytes_read = reader.read_to_string(&mut output).await;

        assert!(bytes_read.is_ok());
        assert_eq!(bytes_read.unwrap(), output.len());
        assert_eq!(output, expected);
    }
}
