use crate::context::Context;
use bytes::Bytes;
use lol_html::OutputSink;

#[derive(Debug)]
pub struct RelaySink {
    context: Context,
}

impl RelaySink {
    pub fn new(context: Context) -> Self {
        Self { context }
    }
}

impl OutputSink for RelaySink {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            self.context.set_done();
        } else {
            let bytes = Bytes::copy_from_slice(chunk);
            self.context.queue().push(bytes);
        }
        self.context.wake();
    }
}

impl Drop for RelaySink {
    fn drop(&mut self) {
        self.context.set_done();
        // Inform everybody that's waiting for us that we're done with producing data
        self.context.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::RelaySink;
    use crate::context::Context;
    use lol_html::OutputSink;

    #[tokio::test]
    async fn handle_chunk() {
        let input: Vec<&[u8]> = vec![b"chunk[0]", b"chunk[1]", b"", b"<invalid>"];
        let expected: Vec<u8> = b"chunk[0]chunk[1]".into();

        let context = Context::default();
        let mut sink = RelaySink::new(context.clone());

        for chunk in input {
            sink.handle_chunk(chunk);
            if context.is_done() {
                break;
            }
        }

        let mut output = Vec::new();
        while let Some(bytes) = context.queue().pop() {
            output.extend_from_slice(&bytes);
        }

        assert_eq!(output, expected);
    }
}