use criterion::{criterion_group, BatchSize, Criterion};

use async_html_rewriter::rewriter::Rewriter;
use async_html_rewriter::settings::Settings;
use lol_html::element;
use lol_html::html_content::ContentType;
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use tokio::task;
use tokio_test::stream_mock::{StreamMock, StreamMockBuilder};

async fn rewrite(mut source: StreamMock<Vec<u8>>, settings: Settings<'static, 'static>) {
    let local_set = task::LocalSet::new();
    local_set.run_until(async move {
        let rewriter = Rewriter::new(settings);
        let mut stream = rewriter.output_reader();
        let rewriter_handle = task::spawn_local(async move {
            rewriter.rewrite(&mut source).await
        });
        let mut buf = String::new();
        stream.read_to_string(&mut buf).await.unwrap();
        let _ = rewriter_handle.await.unwrap();
    }).await;
}

fn bench_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("end-to-end", move |b| {
        b.to_async(&rt).iter_batched(
            || {
                let source = StreamMockBuilder::new()
                    .next(b"<h1>Benchmark</h1>".into())
                    .build();
                let mut settings = Settings::new();
                settings.element_content_handlers = vec![
                    element!("h1", |el| {
                            el.replace("<h1>Benchmark</h1>", ContentType::Html);
                            Ok(())
                        })
                ];
                (source, settings)
            },
            |(source, settings)| async {
                rewrite(source, settings).await
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_throughput);
