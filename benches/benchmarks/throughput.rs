use criterion::{criterion_group, BatchSize, Criterion};

use async_html_rewriter::rewriter::Rewriter;
use async_html_rewriter::settings::Settings;
use lol_html::element;
use lol_html::html_content::ContentType;
use tokio::io::AsyncReadExt;
use tokio::runtime::Runtime;
use tokio::task;
use tokio_test::io::Mock;

async fn rewrite(source: Mock, settings: Settings<'static, 'static>) {
    let local_set = task::LocalSet::new();
    local_set.run_until(async move {
        let rewriter = Rewriter::new(source, settings);
        let mut stream = rewriter.output_reader();
        let rewriter_handle = task::spawn_local(rewriter);
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
                let source = tokio_test::io::Builder::new()
                    .read(b"<h1>Benchmark</h1>")
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
