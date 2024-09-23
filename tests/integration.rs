use async_html_rewriter::rewriter::Rewriter;
use async_html_rewriter::settings::Settings;
use lol_html::element;
use lol_html::html_content::ContentType;
use tokio::io::AsyncReadExt;
use tokio::task;
use tokio_test::stream_mock::StreamMockBuilder;

#[tokio::test]
async fn rewrite_html() {
    let expected = "<h1>Succeeded</h1>";

    let mut settings = Settings::new();
    settings.element_content_handlers = vec![element!("h1", |el| {
            el.replace(expected, ContentType::Html);
            Ok(())
    })];
    let mut source = StreamMockBuilder::new()
        .next(b"<h1>Test</h1>")
        .build();

    let local_set = task::LocalSet::new();
    let buffer = local_set.run_until(async move {
        let rewriter = Rewriter::new(settings);
        let mut stream = rewriter.output_reader();

        task::spawn_local(async move {
            let result = rewriter.rewrite_stream(&mut source).await;
            assert!(result.is_ok(), "Expected rewrite operation to succeed: {:?}", result);
        });

        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        buf
    }).await;

    assert_eq!(buffer, expected);
}
