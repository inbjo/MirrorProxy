use super::*;
use crate::{
    build_router,
    config::{Config, DohConfig},
};
use axum::{body::Body, extract::Request, routing::post, Router};
use tower::ServiceExt;

pub(super) fn wire_query() -> Bytes {
    Bytes::from(
        URL_SAFE_NO_PAD
            .decode("AAABAAABAAAAAAAAB2V4YW1wbGUDY29tAAABAAE")
            .unwrap(),
    )
}

pub(super) fn answer() -> Vec<u8> {
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
        "https://1.1.1.1:8443/dns-query",
        "https://user:secret@1.1.1.1/dns-query",
        "https://1.1.1.1/",
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
    for upstreams in [vec![], vec!["https://1.1.1.1/dns-query".into(); 17]] {
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
            .unwrap()
            .wire,
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
                    .method(method.clone())
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
        // The POST must now be served from memory even with the upstream gone.
        if method == Method::GET {
            server.abort();
        }
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

#[test]
fn custom_doh_endpoints_accept_public_hosts_and_paths_but_reject_unsafe_targets() {
    for endpoint in [
        "https://dns.alidns.com/dns-query",
        "https://doh.pub/dns-query",
        "https://doh.dns.sb/dns-query",
        "https://dns.adguard-dns.com/dns-query",
        "https://resolver.example/custom/dns",
        "https://[2606:4700:4700::1111]/dns-query",
    ] {
        DohConfig {
            enabled: true,
            upstreams: vec![endpoint.into()],
        }
        .validate()
        .unwrap();
    }
    for endpoint in [
        "https://10.0.0.1/dns-query",
        "https://169.254.169.254/dns-query",
        "https://100.64.0.1/dns-query",
        "https://168.63.129.16/dns-query",
        "https://[::ffff:127.0.0.1]/dns-query",
        "https://[fc00::1]/dns-query",
        "https://localhost/dns-query",
        "https://service.local/dns-query",
    ] {
        assert!(
            DohConfig {
                enabled: true,
                upstreams: vec![endpoint.into()]
            }
            .validate()
            .is_err(),
            "{endpoint}"
        );
    }
    assert!(DohConfig {
        enabled: false,
        upstreams: vec![]
    }
    .validate()
    .is_ok());
    assert!(DohConfig {
        enabled: true,
        upstreams: vec![]
    }
    .validate()
    .is_err());
    assert!(DohConfig {
        enabled: true,
        upstreams: vec!["https://dns.google/dns-query".into(); 2]
    }
    .validate()
    .is_err());
    let endpoints = (0..16)
        .map(|i| format!("https://resolver{i}.example/dns-query"))
        .collect();
    assert!(DohConfig {
        enabled: true,
        upstreams: endpoints
    }
    .validate()
    .is_ok());
}

#[tokio::test]
async fn doh_dns_connector_rejects_private_and_mixed_answers() {
    use reqwest::dns::Resolve;
    for addresses in [
        vec![],
        vec!["127.0.0.1:443"],
        vec!["1.1.1.1:443", "10.0.0.1:443"],
        vec!["[::1]:443"],
    ] {
        assert!(!safe_addresses(
            &addresses
                .iter()
                .map(|address| address.parse().unwrap())
                .collect::<Vec<_>>()
        ));
    }
    assert!(safe_addresses(&[
        "1.1.1.1:443".parse().unwrap(),
        "[2606:4700:4700::1111]:443".parse().unwrap()
    ]));
    assert!(PublicResolver
        .resolve("localhost".parse().unwrap())
        .await
        .is_err());
}

#[tokio::test]
async fn cached_doh_get_and_post_preserve_rate_limits_and_public_config_hides_upstreams() {
    let application = crate::build_application(Config {
        doh: DohConfig {
            enabled: true,
            ..Default::default()
        },
        rate_limit: crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 2,
        },
        ..Default::default()
    })
    .await
    .unwrap();
    let query = wire_query();
    application.state.doh_cache.insert(
        &application.state.config().doh.upstreams,
        &query,
        &Answer {
            wire: Bytes::from(answer()),
            cache_ttl: Some(60),
        },
        Instant::now(),
    );
    for (index, method) in [Method::GET, Method::POST, Method::GET]
        .into_iter()
        .enumerate()
    {
        let mut query = query.to_vec();
        query[0] = index as u8;
        let uri = if method == Method::GET {
            format!("/dns-query?dns={}", URL_SAFE_NO_PAD.encode(&query))
        } else {
            "/dns-query".into()
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
                    .method(method.clone())
                    .uri(uri)
                    .header(header::CONTENT_TYPE, DNS_MESSAGE)
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if index < 2 {
                StatusCode::OK
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
        if index < 2 {
            let body = to_bytes(response.into_body(), MAX_MESSAGE).await.unwrap();
            assert_eq!(body[..2], query[..2]);
        }
    }
    let response = application
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/public-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65535).await.unwrap()).unwrap();
    assert_eq!(body["doh"], serde_json::json!({"enabled": true}));
    let mut next = (*application.state.config()).clone();
    next.doh.enabled = false;
    *application.state.config.write().unwrap() = std::sync::Arc::new(next);
    let response = application
        .router
        .oneshot(
            Request::builder()
                .uri("/api/public-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65535).await.unwrap()).unwrap();
    assert_eq!(body["doh"], serde_json::json!({"enabled": false}));
}

#[tokio::test]
async fn doh_adds_ecs_from_trusted_transport_and_isolates_cached_subnets() {
    use axum::extract::ConnectInfo;
    use std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
    };
    let observed = Arc::new(Mutex::new(Vec::<Bytes>::new()));
    let captured = observed.clone();
    let upstream = Router::new()
        .route(
            "/wrong-ecs",
            post(|request: Request| async move {
                let wire = to_bytes(request.into_body(), MAX_MESSAGE).await.unwrap();
                let wrong = ecs::prepare(wire_query(), "4.2.2.2".parse().unwrap())
                    .unwrap()
                    .wire;
                let mut response = answer();
                response[..2].copy_from_slice(&wire[..2]);
                response[10..12].copy_from_slice(&wrong[10..12]);
                response.extend_from_slice(&wrong[wire_query().len()..]);
                ([(header::CONTENT_TYPE, DNS_MESSAGE)], response)
            }),
        )
        .route(
            "/dns-query",
            post(move |request: Request| {
                let captured = captured.clone();
                async move {
                    assert!(!request.headers().contains_key(header::AUTHORIZATION));
                    assert!(!request.headers().contains_key(header::COOKIE));
                    assert!(!request.headers().contains_key("x-forwarded-for"));
                    let wire = to_bytes(request.into_body(), MAX_MESSAGE).await.unwrap();
                    assert!(validate_message(&wire, false).is_ok());
                    let count = {
                        let mut observed = captured.lock().unwrap();
                        observed.push(wire.clone());
                        observed.len() as u8
                    };
                    let mut response = answer();
                    response[..2].copy_from_slice(&wire[..2]);
                    *response.last_mut().unwrap() = count;
                    response[10..12].copy_from_slice(&wire[10..12]);
                    response.extend_from_slice(&wire[wire_query().len()..]);
                    ([(header::CONTENT_TYPE, DNS_MESSAGE)], response)
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/dns-query", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let application = crate::build_application(Config {
        doh: DohConfig {
            enabled: true,
            ..Default::default()
        },
        trusted_proxies: vec!["127.0.0.1".into(), "10.0.0.0/8".into()],
        rate_limit: crate::config::RateLimitConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .unwrap();
    // Inject only this deterministic local upstream after normal validation.
    let mut config = (*application.state.config()).clone();
    config.doh.upstreams = vec![endpoint.replace("/dns-query", "/wrong-ecs"), endpoint];
    *application.state.config.write().unwrap() = Arc::new(config);
    for (index, (peer, forwarded, resolved, calls)) in [
        ("8.8.8.25:4000", "9.9.9.25", "8.8.8.25", 1usize),
        ("8.8.8.199:4000", "1.1.1.1", "8.8.8.199", 1),
        ("9.9.9.200:4000", "", "9.9.9.200", 2),
        ("127.0.0.1:4000", "1.1.1.18, 10.2.0.4", "1.1.1.18", 3),
        ("127.0.0.1:4000", "8.8.8.25, 9.9.9.1", "9.9.9.1", 3),
        ("10.0.0.99:4000", "", "10.0.0.99", 4),
        ("127.0.0.1:4000", "spoofed-value", "127.0.0.1", 4),
        (
            "[2606:4700:1234:5678::1]:4000",
            "",
            "2606:4700:1234:5678::1",
            5,
        ),
        (
            "[2606:4700:1234:56ff::2]:4000",
            "",
            "2606:4700:1234:56ff::2",
            5,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let before = observed.lock().unwrap().len();
        let mut original = wire_query().to_vec();
        original[..2].copy_from_slice(&(index as u16 + 100).to_be_bytes());
        let original = Bytes::from(original);
        let method = if index % 2 == 0 {
            Method::GET
        } else {
            Method::POST
        };
        let uri = if method == Method::GET {
            format!("/dns-query?dns={}", URL_SAFE_NO_PAD.encode(&original))
        } else {
            "/dns-query".into()
        };
        let body = if method == Method::POST {
            Body::from(original.clone())
        } else {
            Body::empty()
        };
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, DNS_MESSAGE)
            .header("x-forwarded-for", forwarded)
            .header(header::AUTHORIZATION, "Bearer client-secret")
            .header(header::COOKIE, "client=secret")
            .body(body)
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let response = application.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let result = to_bytes(response.into_body(), MAX_MESSAGE).await.unwrap();
        assert_eq!(&result[..2], &original[..2]);
        assert_eq!(
            &result[10..12],
            &[0, 0],
            "do not expose automatic EDNS/ECS to the caller"
        );
        assert!(validate_message(&result, true).is_ok());
        let observed = observed.lock().unwrap();
        assert_eq!(observed.len(), calls);
        if observed.len() > before {
            assert_eq!(
                observed.last().unwrap(),
                &ecs::prepare(original, resolved.parse().unwrap())
                    .unwrap()
                    .wire
            );
        }
        let expected_answer = match index {
            0 | 1 => 1,
            2 | 4 => 2,
            3 => 3,
            5 | 6 => 4,
            _ => 5,
        };
        assert_eq!(
            *result.last().unwrap(),
            expected_answer,
            "cache must retain the right subnet's answer"
        );
    }
    // Explicit ECS can share the raw cached response with an automatic query,
    // but its caller must still receive the ECS echo.
    let explicit = ecs::prepare(wire_query(), "1.1.1.18".parse().unwrap())
        .unwrap()
        .wire;
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/dns-query")
        .header(header::CONTENT_TYPE, DNS_MESSAGE)
        .body(Body::from(explicit.clone()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("9.9.9.9:4000".parse::<SocketAddr>().unwrap()));
    let response = application.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), MAX_MESSAGE).await.unwrap();
    assert_eq!(&body[10..12], &[0, 1]);
    assert!(ecs::response_matches(&explicit, &body));
    assert_eq!(observed.lock().unwrap().len(), 5);
    server.abort();
}
