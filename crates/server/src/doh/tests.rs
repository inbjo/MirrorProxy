use super::*;
use crate::{
    build_router,
    config::{Config, DohConfig},
};
use axum::{body::Body, extract::Request, routing::post, Router};
use tower::ServiceExt;

fn wire_query() -> Bytes {
    Bytes::from(
        URL_SAFE_NO_PAD
            .decode("AAABAAABAAAAAAAAB2V4YW1wbGUDY29tAAABAAE")
            .unwrap(),
    )
}

fn answer() -> Vec<u8> {
    let mut answer = wire_query().to_vec();
    answer[2] = 0x81;
    answer[3] = 0x80;
    answer[7] = 1;
    answer.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 93, 184, 216, 34]);
    answer
}

#[test]
fn doh_configuration_defaults_roundtrip_and_restricts_targets() {
    let config: Config = toml::from_str("").unwrap();
    assert!(!config.doh.enabled);
    assert_eq!(config.doh.upstreams, ["https://1.1.1.1/dns-query"]);
    let encoded = serde_json::to_string(&config).unwrap();
    assert_eq!(
        serde_json::from_str::<Config>(&encoded).unwrap().doh,
        config.doh
    );
    for host in [
        "1.1.1.1",
        "1.0.0.1",
        "8.8.8.8",
        "8.8.4.4",
        "9.9.9.9",
        "149.112.112.112",
    ] {
        DohConfig {
            enabled: true,
            upstreams: vec![format!("https://{host}/dns-query")],
        }
        .validate()
        .unwrap();
    }
    for url in [
        "http://1.1.1.1/dns-query",
        "https://127.0.0.1/dns-query",
        "https://[::1]/dns-query",
        "https://resolver.example/dns-query",
        "https://1.1.1.1:8443/dns-query",
        "https://user:secret@1.1.1.1/dns-query",
        "https://1.1.1.1/dns-quey",
        "https://1.1.1.1/dns-query?url=http://127.0.0.1",
        "https://1.1.1.1/dns-query#fragment",
    ] {
        assert!(
            DohConfig {
                enabled: true,
                upstreams: vec![url.into()]
            }
            .validate()
            .is_err(),
            "{url}"
        );
    }
    for upstreams in [vec![], vec!["https://1.1.1.1/dns-query".into(); 7]] {
        assert!(DohConfig {
            enabled: true,
            upstreams
        }
        .validate()
        .is_err());
    }
}

#[test]
fn doh_validates_dns_questions_compression_and_record_boundaries() {
    let query = wire_query();
    let response = answer();
    assert_eq!(
        validate_message(&query, false),
        validate_message(&response, true)
    );
    for end in 0..query.len() {
        assert!(validate_message(&query[..end], false).is_err());
    }
    assert!(validate_message(&response[..response.len() - 1], true).is_err());
    assert!(validate_message(&response, false).is_err());
    let mut trailing = query.to_vec();
    trailing.push(0);
    assert!(validate_message(&trailing, false).is_err());
    let mut looped = response.clone();
    looped[query.len() + 1] = query.len() as u8;
    assert!(validate_message(&looped, true).is_err());
    let mut update = query.to_vec();
    update[2] = 0x29;
    assert!(validate_message(&update, false).is_err());
}

#[tokio::test]
async fn doh_route_rejects_disabled_malformed_and_oversized_requests() {
    let disabled = build_router(Config::default()).await.unwrap();
    assert_eq!(
        disabled
            .oneshot(
                Request::builder()
                    .uri("/dns-query")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let app = build_router(Config {
        doh: DohConfig {
            enabled: true,
            ..DohConfig::default()
        },
        ..Config::default()
    })
    .await
    .unwrap();
    for (uri, method, media, body, expected) in [
        (
            "/dns-query",
            "GET",
            DNS_MESSAGE,
            vec![],
            StatusCode::BAD_REQUEST,
        ),
        (
            "/dns-query?dns=%%%",
            "GET",
            DNS_MESSAGE,
            vec![],
            StatusCode::BAD_REQUEST,
        ),
        (
            "/dns-query?dns=AA&dns=AA",
            "GET",
            DNS_MESSAGE,
            vec![],
            StatusCode::BAD_REQUEST,
        ),
        (
            "/dns-query?url=https://127.0.0.1",
            "GET",
            DNS_MESSAGE,
            vec![],
            StatusCode::BAD_REQUEST,
        ),
        (
            "/dns-query",
            "POST",
            "application/dns-json",
            vec![],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            "/dns-query",
            "POST",
            DNS_MESSAGE,
            vec![0; 12],
            StatusCode::BAD_REQUEST,
        ),
        (
            "/dns-query",
            "POST",
            DNS_MESSAGE,
            vec![0; MAX_MESSAGE + 1],
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "/dns-query",
            "PUT",
            DNS_MESSAGE,
            vec![],
            StatusCode::METHOD_NOT_ALLOWED,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .method(method)
                    .header(header::CONTENT_TYPE, media)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{method} {uri}");
    }
    let encoded = URL_SAFE_NO_PAD.encode(wire_query());
    assert_eq!(
        decode_request(
            Request::builder()
                .uri(format!("/dns-query?dns={encoded}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap(),
        wire_query()
    );
    assert_eq!(
        decode_request(
            Request::builder()
                .method("POST")
                .header(header::CONTENT_TYPE, DNS_MESSAGE)
                .body(Body::from(wire_query()))
                .unwrap()
        )
        .await
        .unwrap(),
        wire_query()
    );
}

#[tokio::test]
async fn doh_relay_preserves_payload_fails_over_and_strips_credentials() {
    let expected = wire_query();
    let upstream = Router::new()
        .route(
            "/redirect",
            post(|| async {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "http://127.0.0.1/private")],
                )
            }),
        )
        .route(
            "/wrong-id",
            post(|| async {
                let mut wire = answer();
                wire[0] = 42;
                ([(header::CONTENT_TYPE, DNS_MESSAGE)], wire)
            }),
        )
        .route("/wrong-type", post(|| async { "not DNS" }))
        .route(
            "/oversized",
            post(|| async {
                (
                    [(header::CONTENT_TYPE, DNS_MESSAGE)],
                    vec![0; MAX_MESSAGE + 1],
                )
            }),
        )
        .route(
            "/dns-query",
            post(move |request: Request| {
                let expected = expected.clone();
                async move {
                    for header in [header::AUTHORIZATION, header::COOKIE] {
                        assert!(!request.headers().contains_key(header));
                    }
                    assert_eq!(request.headers()[header::CONTENT_TYPE], DNS_MESSAGE);
                    assert_eq!(request.headers()[header::ACCEPT], DNS_MESSAGE);
                    assert_eq!(
                        to_bytes(request.into_body(), MAX_MESSAGE).await.unwrap(),
                        expected
                    );
                    (
                        [
                            (header::CONTENT_TYPE, DNS_MESSAGE),
                            (header::SET_COOKIE, "upstream=secret"),
                        ],
                        answer(),
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let endpoints: Vec<_> = [
        "redirect",
        "wrong-id",
        "wrong-type",
        "oversized",
        "dns-query",
    ]
    .iter()
    .map(|path| format!("{base}/{path}"))
    .collect();
    let query = wire_query();
    let question = validate_message(&query, false).unwrap();
    assert!(resolve(&client, &endpoints[..4], &query, &question)
        .await
        .is_none());
    assert_eq!(
        resolve(&client, &endpoints, &query, &question)
            .await
            .unwrap(),
        answer()
    );
    // Exercise the real route and middleware with a local upstream injected
    // after normal configuration validation; production accepts only public IPs.
    let application = crate::build_application(Config {
        doh: DohConfig {
            enabled: true,
            ..DohConfig::default()
        },
        ..Config::default()
    })
    .await
    .unwrap();
    let mut local_config = (*application.state.config()).clone();
    local_config.doh.upstreams = vec![format!("{base}/dns-query")];
    *application.state.config.write().unwrap() = std::sync::Arc::new(local_config);
    for method in [Method::GET, Method::POST] {
        let uri = if method == Method::GET {
            format!("/dns-query?dns={}", URL_SAFE_NO_PAD.encode(&query))
        } else {
            "/dns-query".to_string()
        };
        let body = if method == Method::POST {
            Body::from(query.clone())
        } else {
            Body::empty()
        };
        let response = application
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::CONTENT_TYPE, DNS_MESSAGE)
                    .header(header::AUTHORIZATION, "Bearer client-secret")
                    .header(header::COOKIE, "client=secret")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(
            to_bytes(response.into_body(), MAX_MESSAGE).await.unwrap(),
            answer()
        );
    }
    server.abort();
}

#[tokio::test]
async fn doh_reuses_proxy_classification_and_rate_limits() {
    assert_eq!(crate::proxy_target_for_path("/dns-query"), Some("doh"));
    assert!(crate::is_proxy_path("/dns-query"));
    assert_eq!(crate::route_group_for_path("/dns-query"), "/proxy/doh");
    let app = build_router(Config {
        doh: DohConfig {
            enabled: true,
            ..DohConfig::default()
        },
        rate_limit: crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 1,
        },
        ..Config::default()
    })
    .await
    .unwrap();
    for expected in [StatusCode::BAD_REQUEST, StatusCode::TOO_MANY_REQUESTS] {
        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/dns-query")
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            expected
        );
    }
}
