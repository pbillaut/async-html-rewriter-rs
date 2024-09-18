use async_html_rewriter::rewriter::Rewriter;
use async_html_rewriter::settings::Settings;
use lol_html::element;
use lol_html::html_content::ContentType;
use tokio::io::AsyncReadExt;
use tokio::task;

#[tokio::test]
async fn rewrite_html() {
    let expected = "<h1>Succeeded</h1>";

    let mut settings = Settings::new();
    settings.element_content_handlers = vec![element!("h1", |el| {
            el.replace(expected, ContentType::Html);
            Ok(())
    })];
    let source = tokio_test::io::Builder::new()
        .read(b"<h1>Test</h1>")
        .build();

    let local_set = task::LocalSet::new();
    let buffer = local_set.run_until(async move {
        let rewriter = Rewriter::new(source, settings);
        let mut stream = rewriter.output_stream();

        let rewriter_handle = task::spawn_local(rewriter);

        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        let _ = rewriter_handle.await.unwrap();
        buf
    }).await;

    assert_eq!(buffer, expected);
}
