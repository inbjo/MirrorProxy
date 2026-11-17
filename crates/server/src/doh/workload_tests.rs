//! Local protocol acceptance and repeatable baselines. No external resolver is used.
use super::*;
use axum::{body::Body, extract::ConnectInfo, routing::post, Router};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

async fn fixture() -> (
    crate::BuiltApplication,
    tokio::task::JoinHandle<()>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let upstream = Router::new().route(
        "/dns-query",
        post(move |request: Request| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::Relaxed);
                let wire = to_bytes(request.into_body(), MAX_MESSAGE).await.unwrap();
                validate_message(&wire, false).unwrap();
                let kind =
                    u16::from_be_bytes(wire[wire.len() - 4..wire.len() - 2].try_into().unwrap());
                let mut answer = wire.to_vec();
                answer[2] |= 0x80;
                answer[3] = 0x80;
                if wire.windows(7).any(|label| label == b"invalid") {
                    answer[3] |= 3;
                    answer[9] = 1;
                    // SOA negative caching: root owner/MNAME/RNAME, five u32 fields.
                    answer.extend_from_slice(&[0, 0, 6, 0, 1, 0, 0, 0, 60, 0, 22, 0, 0]);
                    for value in [1u32, 60, 60, 60, 60] {
                        answer.extend_from_slice(&value.to_be_bytes());
                    }
                } else {
                    answer[7] = 1;
                    let data = match kind {
                        1 => vec![93, 184, 216, 34],
                        28 => vec![0x20; 16],
                        65 => vec![0, 1, 0],
                        _ => panic!("unexpected query"),
                    };
                    answer.extend_from_slice(&[0xc0, 12]);
                    answer.extend_from_slice(&kind.to_be_bytes());
                    answer.extend_from_slice(&[0, 1, 0, 0, 0, 60]);
                    answer.extend_from_slice(&(data.len() as u16).to_be_bytes());
                    answer.extend_from_slice(&data);
                }
                ([(header::CONTENT_TYPE, DNS_MESSAGE)], answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/dns-query", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let mut config = crate::config::Config::default();
    config.doh.enabled = true;
    let application = crate::build_application(config).await.unwrap();
    let mut config = (*application.state.config()).clone();
    config.doh.upstreams = vec![endpoint];
    application.state.runtime.write().unwrap().config = Arc::new(config);
    (application, task, calls)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Linux CI protocol acceptance invokes Python; run explicitly"]
async fn local_doh_protocol_smoke() {
    let (application, upstream, calls) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            application
                .router
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/smoke-doh.py"
            ))
            .arg(base)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    server.abort();
    upstream.abort();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        4,
        "POST must reuse GET results, including negative cache"
    );
    println!("{}", String::from_utf8_lossy(&result.stdout));
}

fn rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn summarize(mut samples: Vec<u64>) -> serde_json::Value {
    samples.sort_unstable();
    let percentile = |p: usize| samples[(samples.len() - 1) * p / 100];
    serde_json::json!({"requests":samples.len(),"p50_us":percentile(50),"p95_us":percentile(95),"p99_us":percentile(99)})
}
async fn lookup(router: Router, hostname: &str, id: u16, ip: u8) -> u64 {
    let mut wire = health::probe_query(hostname, 1).to_vec();
    wire[..2].copy_from_slice(&id.to_be_bytes());
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/dns-query")
        .header(header::CONTENT_TYPE, DNS_MESSAGE)
        .body(Body::from(wire.clone()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        format!("8.8.8.{ip}:4000")
            .parse::<std::net::SocketAddr>()
            .unwrap(),
    ));
    let start = Instant::now();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let answer = to_bytes(response.into_body(), MAX_MESSAGE).await.unwrap();
    assert_eq!(answer[..2], wire[..2]);
    assert_eq!(
        validate_message(&answer, true),
        validate_message(&wire, false)
    );
    assert_eq!(&answer[answer.len() - 4..], &[93, 184, 216, 34]);
    start.elapsed().as_micros() as u64
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "capacity baseline is run explicitly and reports timings without machine-dependent gates"]
async fn doh_capacity_baseline() {
    let (application, upstream, calls) = fixture().await;
    let rss_before = rss_kib();
    let started = Instant::now();
    let mut misses = Vec::new();
    let mut hits = Vec::new();
    for index in 0..128 {
        misses.push(
            lookup(
                application.router.clone(),
                &format!("q{index}.example.com"),
                index,
                1,
            )
            .await,
        );
    }
    for index in 0..1024 {
        hits.push(
            lookup(
                application.router.clone(),
                &format!("q{}.example.com", index % 128),
                index,
                (index % 64 + 1) as u8,
            )
            .await,
        );
    }
    let mut workers = Vec::new();
    for index in 0..64 {
        let router = application.router.clone();
        workers.push(tokio::spawn(async move {
            lookup(router, "q0.example.com", index, index as u8 + 1).await
        }));
    }
    let mut parallel = Vec::new();
    for task in workers {
        parallel.push(task.await.unwrap());
    }
    let report = application
        .state
        .doh_cache
        .runtime
        .report(&application.state.config().doh);
    assert_eq!(calls.load(Ordering::Relaxed), 128);
    assert_eq!(report.cache_hits, 1088);
    assert_eq!(report.cache_misses, 128);
    assert!(application
        .state
        .doh_cache
        .runtime
        .flights
        .lock()
        .unwrap()
        .is_empty());
    assert_eq!(
        application
            .state
            .doh_cache
            .runtime
            .requests
            .available_permits(),
        256
    );
    let report = serde_json::json!({"schema":1,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"version":env!("CARGO_PKG_VERSION"),"ips":64,"cache_miss":summarize(misses),"cache_hit":summarize(hits),"concurrent_hits":summarize(parallel),"upstream_requests":128,"rss_before_kib":rss_before,"rss_after_kib":rss_kib(),"elapsed_ms":started.elapsed().as_millis()});
    if let Ok(path) = std::env::var("MIRRORPROXY_BASELINE_PATH") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{report}");
    upstream.abort();
}
