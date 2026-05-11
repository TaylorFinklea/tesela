//! End-to-end HTTP bench — spawns `tesela-server` against a synthetic
//! mosaic, times a handful of representative requests. Catches
//! regressions in the full stack: file walk → SQLite query → serde →
//! HTTP serialization → axum routing.
//!
//! `cargo bench --bench http -p tesela-server`
//!
//! Bench groups:
//! - `server/http/list_notes_limit_60`     — the Dailies fetch shape
//! - `server/http/types_task_blocks`       — exercise the unbounded
//!                                            `get_typed_blocks` query
//! - `server/http/imports_logseq_plan`     — plan an import (creates
//!                                            no files; pure read +
//!                                            convert path)

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tesela_fixtures::{MosaicBuilder, MosaicHandle};
use tokio::runtime::Runtime;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_tesela-server"))
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn wait_for_port(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

struct ServerHandle {
    child: Child,
    addr: String,
    _mosaic: MosaicHandle,
}

impl ServerHandle {
    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_with_mosaic(mosaic: MosaicHandle) -> ServerHandle {
    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let child = Command::new(binary_path())
        .current_dir(&mosaic.path)
        .env("TESELA_SERVER_BIND", &addr)
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");
    if !wait_for_port(&addr, Duration::from_secs(30)) {
        panic!("server never bound to {}", addr);
    }
    ServerHandle {
        child,
        addr,
        _mosaic: mosaic,
    }
}

fn server_http(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // One server boot, shared across all sub-benches. A 500-note mosaic
    // takes ~5s to index + bind, so we don't want to pay that per
    // bench function.
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(420)
        .pages(80)
        .tasks(200)
        .backlinks_per_note(1, 4)
        .build()
        .unwrap();
    let server = spawn_with_mosaic(mosaic);

    let mut group = c.benchmark_group("server/http");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(20);

    for limit in &[30u32, 60, 200] {
        let url = server.url(&format!("/notes?tag=daily&limit={}", limit));
        group.bench_with_input(
            BenchmarkId::new("list_notes_daily", limit),
            &url,
            |b, url| {
                b.iter(|| {
                    rt.block_on(async {
                        let resp = client.get(url).send().await.unwrap();
                        let _bytes = resp.bytes().await.unwrap();
                    })
                });
            },
        );
    }

    let url = server.url("/types/Task/blocks");
    group.bench_function("types_task_blocks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resp = client.get(&url).send().await.unwrap();
                let _bytes = resp.bytes().await.unwrap();
            })
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = server_http
}
criterion_main!(benches);
