use axum::{
    body::{to_bytes, Body},
    extract::Extension,
    http::{HeaderMap, HeaderValue, Request, StatusCode},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tower::ServiceExt;

use super::*;
use crate::database::ProxyTrafficRecord;
use crate::geoip::GeoLocation;

fn assert_security_headers(headers: &HeaderMap, expect_hsts: bool) {
    assert_eq!(
        headers.get("content-security-policy"),
        Some(&HeaderValue::from_static(CONTENT_SECURITY_POLICY))
    );
    assert_eq!(
        headers.get("x-frame-options"),
        Some(&HeaderValue::from_static("DENY"))
    );
    assert_eq!(
        headers.get("x-content-type-options"),
        Some(&HeaderValue::from_static("nosniff"))
    );
    assert_eq!(
        headers.get("referrer-policy"),
        Some(&HeaderValue::from_static("strict-origin-when-cross-origin"))
    );
    assert_eq!(
        headers.get("permissions-policy"),
        Some(&HeaderValue::from_static(PERMISSIONS_POLICY))
    );
    assert_eq!(
        headers
            .get("strict-transport-security")
            .and_then(|value| value.to_str().ok()),
        expect_hsts.then_some(STRICT_TRANSPORT_SECURITY)
    );
}

#[tokio::test]
async fn responses_include_security_headers_and_https_enables_hsts() {
    let app = build_router(Config {
        public_base_url: "https://mirror.example".to_string(),
        ..Config::default()
    })
    .await
    .unwrap();

    for uri in ["/", "/api/public-config"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("https://mirror.example{uri}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_security_headers(response.headers(), true);
    }
}

#[tokio::test]
async fn hsts_requires_a_verified_https_delivery_path() {
    let app = build_router(Config::default()).await.unwrap();
    let mut untrusted_request = Request::builder()
        .uri("/healthz")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    untrusted_request.extensions_mut().insert(ConnectInfo(
        "198.51.100.10:4242".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(untrusted_request).await.unwrap();
    assert_security_headers(response.headers(), false);

    let mut trusted_request = Request::builder()
        .uri("/healthz")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    trusted_request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4242".parse::<SocketAddr>().unwrap()));
    let response = app.oneshot(trusted_request).await.unwrap();
    assert_security_headers(response.headers(), true);
}

#[test]
fn https_detection_uses_the_connection_instead_of_public_url_or_acme_settings() {
    let mut config = Config {
        public_base_url: "https://mirror.example".to_string(),
        ..Config::default()
    };
    config.acme.direct_https = true;
    let mut request = Request::builder()
        .uri("/admin")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.10:4242".parse::<SocketAddr>().unwrap(),
    ));
    assert!(!request_uses_https(&request, &config));
    request.extensions_mut().insert(HttpsTransport);
    assert!(request_uses_https(&request, &config));
}

#[tokio::test]
async fn connection_diagnostics_explain_untrusted_https_without_granting_trust() {
    let (state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let peer: SocketAddr = "192.168.64.1:4242".parse().unwrap();
    let app = Router::new()
        .route("/admin/api/connection-info", get(admin_connection_info))
        .route(
            "/admin/api/probe",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .layer(middleware::from_fn(csrf_protection_middleware))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            strip_untrusted_forwarded_headers,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_headers_middleware,
        ))
        .with_state(state.clone());
    let make_request =
        |path: &str, method: Method, cookie: Option<String>, proof: Option<String>| {
            let mut builder = Request::builder()
                .method(method)
                .uri(path)
                .header(header::HOST, "mirror.example")
                .header(header::ORIGIN, "https://mirror.example")
                .header("x-forwarded-host", "mirror.example")
                .header("x-forwarded-proto", "https")
                .header("x-forwarded-for", "203.0.113.99");
            if let Some(cookie) = cookie {
                builder = builder.header(header::COOKIE, cookie);
            }
            if let Some(proof) = proof {
                builder = builder.header(CSRF_HEADER, proof);
            }
            let mut request = builder.body(Body::empty()).unwrap();
            request.extensions_mut().insert(ConnectInfo(peer));
            request
        };
    let response = app
        .clone()
        .oneshot(make_request(
            "/admin/api/connection-info",
            Method::GET,
            None,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let http_cookie = format!("mirrorproxy_admin_session_http={}", session.token);
    let response = app
        .clone()
        .oneshot(make_request(
            "/admin/api/connection-info",
            Method::GET,
            Some(http_cookie.clone()),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        info,
        serde_json::json!({"peer_ip":"192.168.64.1","trusted_proxy":false,"forwarded_https":true,"effective_https":false})
    );
    assert!(!String::from_utf8_lossy(&body).contains(&session.token));
    assert!(!state.config().is_trusted_proxy(peer.ip()));
    let response = app
        .clone()
        .oneshot(make_request(
            "/admin/api/probe",
            Method::POST,
            Some(http_cookie),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "csrf_invalid");
    assert!(error.get("peer_ip").is_none());
    // Only an explicit administrator configuration change establishes trust.
    Arc::make_mut(&mut state.config.write().unwrap())
        .trusted_proxies
        .push(peer.ip().to_string());
    let https_cookie = format!("{ADMIN_SESSION_COOKIE}={}", session.token);
    let response = app
        .clone()
        .oneshot(make_request(
            "/admin/api/connection-info",
            Method::GET,
            Some(https_cookie.clone()),
            None,
        ))
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(info["trusted_proxy"], true);
    assert_eq!(info["effective_https"], true);
    for (proof, expected) in [
        ("wrong".into(), StatusCode::FORBIDDEN),
        (csrf_token(&session.token), StatusCode::NO_CONTENT),
    ] {
        let response = app
            .clone()
            .oneshot(make_request(
                "/admin/api/probe",
                Method::POST,
                Some(https_cookie.clone()),
                Some(proof),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn browser_login_round_trip_supports_http_and_preserves_https_cookie_security() {
    for secure in [false, true] {
        let (state, credentials) = admin_test_state().await;
        let app = Router::new()
            .route("/admin/api/auth/login", post(admin_cookie_login))
            .route("/admin/api/auth/session", get(admin_session))
            .route("/admin/api/auth/logout", post(admin_logout))
            .layer(middleware::from_fn(csrf_protection_middleware))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                strip_untrusted_forwarded_headers,
            ))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                security_headers_middleware,
            ))
            .with_state(state);
        let origin = if secure {
            "https://mirror.example"
        } else {
            "http://mirror.example:8000"
        };
        let peer = ConnectInfo("198.51.100.10:4242".parse::<SocketAddr>().unwrap());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(format!("{origin}/admin/api/auth/login"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(peer)
                    .body(Body::from(
                        serde_json::json!({
                            "username": credentials.username,
                            "password": credentials.password,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(cookies.len(), 2);
        let session_name = if secure {
            ADMIN_SESSION_COOKIE
        } else {
            "mirrorproxy_admin_session_http"
        };
        let csrf_name = if secure {
            ADMIN_CSRF_COOKIE
        } else {
            "mirrorproxy_admin_csrf_http"
        };
        assert!(cookies[0].starts_with(&format!("{session_name}=")));
        assert!(cookies[1].starts_with(&format!("{csrf_name}=")));
        assert!(cookies[0].contains("HttpOnly"));
        for cookie in &cookies {
            assert_eq!(cookie.contains("; Secure"), secure);
            assert!(cookie.contains("SameSite=Strict"));
            assert!(!cookie.contains("Domain="));
        }
        let cookie_header = cookies
            .iter()
            .map(|cookie| cookie.split(';').next().unwrap())
            .collect::<Vec<_>>()
            .join("; ");
        let csrf = cookies[1]
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1;
        let request = |method: Method, path: &str| {
            Request::builder()
                .method(method)
                .uri(format!("{origin}{path}"))
                .header(
                    header::HOST,
                    if secure {
                        "mirror.example"
                    } else {
                        "mirror.example:8000"
                    },
                )
                .header(header::COOKIE, &cookie_header)
                .extension(peer)
        };
        let response = app
            .clone()
            .oneshot(
                request(Method::GET, "/admin/api/auth/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|value| value
                .to_str()
                .unwrap()
                .starts_with(&format!("{csrf_name}="))));
        // Both transports still require same-origin, session-bound CSRF proof.
        for (supplied_origin, proof) in [(origin, ""), ("http://attacker.example", csrf)] {
            let response = app
                .clone()
                .oneshot(
                    request(Method::POST, "/admin/api/auth/logout")
                        .header(header::ORIGIN, supplied_origin)
                        .header(CSRF_HEADER, proof)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        let response = app
            .clone()
            .oneshot(
                request(Method::POST, "/admin/api/auth/logout")
                    .header(header::ORIGIN, origin)
                    .header(CSRF_HEADER, csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        for cookie in response.headers().get_all(header::SET_COOKIE) {
            let value = cookie.to_str().unwrap();
            assert!(value.contains("Max-Age=0"));
            assert_eq!(value.contains("; Secure"), secure);
        }
        let response = app
            .oneshot(
                request(Method::GET, "/admin/api/auth/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[test]
fn browser_cookies_are_isolated_between_http_and_https() {
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, HeaderValue::from_static(
        "mirrorproxy_admin_session=secure-session; mirrorproxy_admin_session_http=http-session; mirrorproxy_user_session=secure-user; mirrorproxy_user_session_http=http-user; other=preserved",
    ));
    let mut http_headers = headers.clone();
    normalize_browser_session_cookies(&mut http_headers, false);
    assert_eq!(
        cookie_value(&http_headers, ADMIN_SESSION_COOKIE),
        Some("http-session")
    );
    assert_eq!(
        cookie_value(&http_headers, USER_SESSION_COOKIE),
        Some("http-user")
    );
    normalize_browser_session_cookies(&mut headers, true);
    assert_eq!(
        cookie_value(&headers, ADMIN_SESSION_COOKIE),
        Some("secure-session")
    );
    assert_eq!(
        cookie_value(&headers, USER_SESSION_COOKIE),
        Some("secure-user")
    );
    assert_eq!(
        cookie_value(&headers, "mirrorproxy_admin_session_http"),
        None
    );
    assert_eq!(cookie_value(&headers, "other"), Some("preserved"));
    let mut headers = HeaderMap::new();
    headers.append(header::SET_COOKIE, user_session_cookie("test-user-session"));
    headers.append(header::SET_COOKIE, user_csrf_cookie("test-user-session"));
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_static("unrelated=value; Secure"),
    );
    adapt_http_browser_cookies(&mut headers);
    let cookies = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect::<Vec<_>>();
    assert!(cookies[0].starts_with("mirrorproxy_user_session_http="));
    assert!(cookies[0].contains("HttpOnly"));
    assert!(cookies[0].contains("SameSite=Lax"));
    assert!(!cookies[0].contains("Secure"));
    assert!(cookies[1].starts_with("mirrorproxy_user_csrf_http="));
    assert_eq!(cookies[2], "unrelated=value; Secure");
}

#[tokio::test]
async fn cookie_authenticated_mutations_require_same_origin_csrf_proof() {
    let app = Router::new()
        .route(
            "/admin/api/acme/renew",
            post(|| async { StatusCode::ACCEPTED }),
        )
        .layer(middleware::from_fn(csrf_protection_middleware));
    let session = "test-admin-session";
    let valid_csrf = csrf_token(session);

    for (origin, csrf, expected) in [
        (
            "https://tenant.mirror.example",
            valid_csrf.as_str(),
            StatusCode::FORBIDDEN,
        ),
        ("https://mirror.example", "", StatusCode::FORBIDDEN),
        (
            "https://mirror.example",
            valid_csrf.as_str(),
            StatusCode::ACCEPTED,
        ),
    ] {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/admin/api/acme/renew")
            .header(header::HOST, "mirror.example")
            .header(header::ORIGIN, origin)
            .header(header::COOKIE, format!("{ADMIN_SESSION_COOKIE}={session}"));
        if !csrf.is_empty() {
            request = request.header(CSRF_HEADER, csrf);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn bearer_only_control_mutations_do_not_require_browser_csrf_proof() {
    let app = Router::new()
        .route(
            "/admin/api/acme/renew",
            post(|| async { StatusCode::ACCEPTED }),
        )
        .layer(middleware::from_fn(csrf_protection_middleware));
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/api/acme/renew")
                .header(header::AUTHORIZATION, "Bearer service-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[test]
fn systemd_unit_uses_explicit_paths_and_optional_low_port_capability() {
    let unit = render_systemd_unit(
        Path::new("/opt/mirrorproxy/mirrorproxy-server"),
        Path::new("/etc/mirrorproxy/config.toml"),
        Path::new("/var/lib/mirrorproxy"),
        "mirrorproxy",
        true,
    )
    .unwrap();

    assert!(unit.contains("User=mirrorproxy"));
    assert!(unit.contains("WorkingDirectory=/var/lib/mirrorproxy"));
    assert!(unit.contains(
        "ExecStart=/opt/mirrorproxy/mirrorproxy-server --config /etc/mirrorproxy/config.toml serve"
    ));
    assert!(unit.contains("AmbientCapabilities=CAP_NET_BIND_SERVICE"));
    assert!(unit.contains("ReadWritePaths=/var/lib/mirrorproxy"));
    assert!(systemd_text("contains space").is_err());
    assert_eq!(
        systemd_unit_name(Path::new("/etc/systemd/system/mirrorproxy-alt.service")).unwrap(),
        "mirrorproxy-alt.service"
    );
    assert!(systemd_unit_name(Path::new("/tmp/mirrorproxy.unit")).is_err());
}

#[tokio::test]
async fn local_registration_respects_policy_origin_and_password_validation() {
    use tower::ServiceExt;
    let (state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let app = Router::new()
        .route(
            "/admin/api/registration-invitations",
            post(local_accounts::invite),
        )
        .route("/api/auth/register", post(local_accounts::register))
        .route("/api/auth/password/login", post(local_accounts::login))
        .layer(axum::Extension(ConnectInfo(
            "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
        )))
        .with_state(state.clone());
    let billing = serde_json::json!({"quota_mode":"default","group_id":null,"monthly_gb":null});
    Arc::make_mut(&mut state.config.write().unwrap())
        .registration
        .mode = "disabled".into();
    for (path, body, expected) in [
        (
            "/admin/api/registration-invitations",
            serde_json::json!({"billing":billing,"expires_in_hours":72}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/api/auth/register",
            serde_json::json!({"token":"x".repeat(48),"username":"friend","password":"a-unique-user-passphrase","display_name":"Friend"}),
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("host", "mirror.example")
                    .header("origin", "https://mirror.example")
                    .header("content-type", "application/json")
                    .header(
                        "cookie",
                        format!("{ADMIN_SESSION_COOKIE}={}", session.token),
                    )
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    Arc::make_mut(&mut state.config.write().unwrap())
        .registration
        .mode = "invite_only".into();
    for (origin, password, expected) in [
        (
            "https://attacker.example",
            "a-unique-user-passphrase",
            StatusCode::FORBIDDEN,
        ),
        ("https://mirror.example", "", StatusCode::BAD_REQUEST),
    ] {
        let response = app.clone().oneshot(Request::builder().method("POST").uri("/api/auth/register").header("host","mirror.example").header("origin",origin).header("content-type","application/json").body(Body::from(serde_json::json!({"token":"x".repeat(48),"username":"friend","password":password,"display_name":"Friend"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn invitation_registration_defaults_optional_display_name_to_account() {
    use tower::ServiceExt;
    let (state, _) = admin_test_state().await;
    Arc::make_mut(&mut state.config.write().unwrap())
        .registration
        .mode = "invite_only".into();
    let app = Router::new()
        .route("/api/auth/register", post(local_accounts::register))
        .layer(axum::Extension(ConnectInfo(
            "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
        )))
        .with_state(state.clone());
    for (index, name) in [None, Some(""), Some("   "), Some("  自定义名称  ")]
        .into_iter()
        .enumerate()
    {
        let (_, token) = state
            .database
            .create_registration_invitation(
                "admin",
                &database::local_accounts::AccountBilling::default(),
                72,
            )
            .await
            .unwrap()
            .unwrap();
        let username = format!("invited-{index}");
        let mut body = serde_json::json!({
            "token": token, "username": format!(" {username} "),
            "password": uuid::Uuid::new_v4().to_string(),
        });
        if let Some(name) = name {
            body["display_name"] = name.into();
        }
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("host", "mirror.example")
                    .header("origin", "https://mirror.example")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
        let user = state
            .database
            .user_account(response["user_id"].as_i64().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.username.as_deref(), Some(username.as_str()));
        assert_eq!(
            user.display_name,
            if index == 3 {
                "自定义名称"
            } else {
                &username
            }
        );
    }
}

async fn admin_test_state() -> (AppState, database::InitialAdminCredentials) {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(Config::default()))),
        database: Arc::new(database),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    (state, credentials.unwrap())
}

#[tokio::test]
async fn direct_http_preserves_acme_path_and_redirects_to_https() {
    let (acme, _) = acme::AcmeManager::new(AcmeConfig::default());
    let ready = Arc::new(AtomicBool::new(true));
    let app = Router::new()
        .route(
            "/.well-known/acme-challenge/{token}",
            get(direct_acme_http01_challenge),
        )
        .fallback(direct_http_redirect)
        .with_state(DirectHttpState {
            acme,
            domains: Arc::new(vec![
                "mirror.example.com".to_string(),
                "*.mirror.example.com".to_string(),
            ]),
            https_addr: "127.0.0.1:8443".parse().unwrap(),
            https_ready: ready.clone(),
        });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/npm/package?version=1")
                .header(header::HOST, "mirror.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers()[header::LOCATION],
        "https://mirror.example.com:8443/npm/package?version=1"
    );

    let challenge = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/.well-known/acme-challenge/missing")
                .header(header::HOST, "mirror.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(challenge.status(), StatusCode::NOT_FOUND);

    ready.store(false, Ordering::Release);
    let provisioning = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "mirror.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(provisioning.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(provisioning.headers()[header::RETRY_AFTER], "5");
}

#[test]
fn direct_https_redirect_rejects_unconfigured_and_nested_wildcard_hosts() {
    let domains = vec![
        "mirror.example.com".to_string(),
        "*.example.com".to_string(),
    ];
    let uri = "/healthz".parse::<Uri>().unwrap();
    for host in ["other.example.net", "nested.user.example.com"] {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        assert_eq!(
            direct_https_location(&headers, &uri, &domains, 443),
            Err(StatusCode::MISDIRECTED_REQUEST)
        );
    }
}

#[tokio::test]
async fn native_https_serves_with_an_existing_certificate() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (state, credentials) = admin_test_state().await;
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/admin/api/auth/login", post(admin_cookie_login))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_headers_middleware,
        ))
        .with_state(state);
    let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("generate test certificate");
    let directory =
        std::env::temp_dir().join(format!("mirrorproxy-native-https-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("fullchain.pem"), generated.cert.pem()).unwrap();
    fs::write(
        directory.join("privkey.pem"),
        generated.signing_key.serialize_pem(),
    )
    .unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = axum_server::Handle::new();
    let ready = Arc::new(AtomicBool::new(false));
    let (acme, _) = acme::AcmeManager::new(AcmeConfig {
        enabled: true,
        direct_https: true,
        storage_directory: directory.display().to_string(),
        ..AcmeConfig::default()
    });
    let task = tokio::spawn(run_https_listener(
        listener,
        handle.clone(),
        router,
        acme.clone(),
        directory.clone(),
        ready.clone(),
        address,
    ));

    timeout(Duration::from_secs(3), async {
        while !ready.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("HTTPS listener should become ready");
    let base_url = format!("https://localhost:{}", address.port());
    let certificate = Certificate::from_pem(generated.cert.pem().as_bytes()).unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("localhost", address)
        .add_root_certificate(certificate)
        .build()
        .unwrap();
    Client::builder()
        .no_proxy()
        .resolve("localhost", address)
        .build()
        .unwrap()
        .get(format!("{base_url}/healthz"))
        .send()
        .await
        .expect_err("the self-signed certificate must require explicit trust");
    client
        .get(format!("https://{address}/healthz"))
        .send()
        .await
        .expect_err("trusting the certificate must not bypass hostname verification");
    let response = client
        .get(format!("{base_url}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.text().await.unwrap(), "ok");

    let response = client
        .post(format!("{base_url}/admin/api/auth/login"))
        .json(&serde_json::json!({
            "username": credentials.username,
            "password": credentials.password,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_security_headers(response.headers(), true);
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect::<Vec<_>>();
    assert!(cookies[0].starts_with("mirrorproxy_admin_session="));
    assert!(cookies[1].starts_with("__Host-mirrorproxy_admin_csrf="));
    assert!(cookies.iter().all(|value| value.contains("; Secure")));

    let renewed = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("generate renewed test certificate");
    let renewed_pem = renewed.cert.pem();
    fs::write(directory.join("fullchain.pem"), &renewed_pem).unwrap();
    fs::write(
        directory.join("privkey.pem"),
        renewed.signing_key.serialize_pem(),
    )
    .unwrap();
    acme.notify_certificate_update();
    let renewed_certificate = Certificate::from_pem(renewed_pem.as_bytes()).unwrap();
    let renewed_client = Client::builder()
        .no_proxy()
        .resolve("localhost", address)
        .add_root_certificate(renewed_certificate)
        .build()
        .unwrap();
    timeout(Duration::from_secs(3), async {
        loop {
            if renewed_client
                .get(format!("{base_url}/healthz"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("renewed certificate should be hot-reloaded");

    handle.shutdown();
    timeout(Duration::from_secs(3), task)
        .await
        .expect("HTTPS listener should stop")
        .unwrap()
        .unwrap();
    let _ = fs::remove_dir_all(directory);
}

async fn routing_test_state(mode: &str) -> (AppState, database::UserAccount) {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let user = database
        .create_user("admin", "person@example.com", "Person", 12)
        .await
        .unwrap()
        .unwrap();
    let config = Config {
        public_base_url: "https://mirror.example.com".to_string(),
        user_access: crate::config::UserAccessConfig {
            base_domain: "mirror.example.com".to_string(),
            mode: mode.to_string(),
            infrastructure_ready: mode == "subdomain_required",
            ..Default::default()
        },
        ..Config::default()
    };
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(config))),
        database: Arc::new(database),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    (state, user)
}

async fn read_http_headers(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

async fn respond_ok(stream: &mut TcpStream) {
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .await
        .unwrap();
}

// The proxy mocks tunnel to a real TLS origin. Trust only its generated
// certificate, so SOCKS DNS/authentication tests also retain TLS validation.
async fn proxy_test_tls_origin(
    host: &str,
) -> (SocketAddr, Certificate, axum_server::Handle<SocketAddr>) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let generated = rcgen::generate_simple_self_signed(vec![host.to_string()]).unwrap();
    let certificate = Certificate::from_pem(generated.cert.pem().as_bytes()).unwrap();
    let tls = axum_server::tls_rustls::RustlsConfig::from_pem(
        generated.cert.pem().into_bytes(),
        generated.signing_key.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = axum_server::Handle::new();
    let server_handle = handle.clone();
    tokio::spawn(async move {
        axum_server::from_tcp_rustls(listener, tls)
            .unwrap()
            .handle(server_handle)
            .serve(
                Router::new()
                    .route("/from-socks", get(|| async { "ok" }))
                    .into_make_service(),
            )
            .await
            .unwrap();
    });
    (address, certificate, handle)
}

#[test]
fn generated_admin_credentials_log_contains_username_and_password() {
    let generated_password = uuid::Uuid::new_v4().to_string();
    let message = initial_admin_credentials_log("admin", &generated_password, true);

    let mut lines = message.lines();
    assert_eq!(lines.next(), Some(""));
    assert_eq!(lines.next(), Some("INITIAL ADMIN USERNAME: admin"));
    let expected_password_line = format!("INITIAL ADMIN PASSWORD: {generated_password}");
    assert_eq!(lines.next(), Some(expected_password_line.as_str()));
}

#[test]
fn configured_admin_password_is_not_printed() {
    let configured_password = uuid::Uuid::new_v4().to_string();
    let message = initial_admin_credentials_log("admin", &configured_password, false);
    assert!(message.contains("INITIAL ADMIN USERNAME: admin"));
    assert!(message.contains("configured by MIRRORPROXY_ADMIN_PASSWORD (not shown)"));
    assert!(!message.contains(&configured_password));
}

#[test]
fn parses_admin_password_reset_command_without_a_username() {
    let cli = Cli::try_parse_from(["mirrorproxy-server", "admin", "reset-password"]).unwrap();
    let Some(Command::Admin {
        command: AdminCommand::ResetPassword,
    }) = cli.command
    else {
        panic!("password reset command was not parsed");
    };
}

#[tokio::test]
async fn global_http_proxy_handles_upstream_requests_and_authentication() {
    use base64::Engine as _;
    let proxy_password = uuid::Uuid::new_v4().to_string();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_headers(&mut stream).await;
        respond_ok(&mut stream).await;
        request
    });
    let config = Config {
        outbound_proxy: OutboundProxyConfig {
            enabled: true,
            url: format!("http://{address}"),
            no_proxy: Vec::new(),
            username: Some("proxy-user".to_string()),
            password: Some(proxy_password.clone()),
        },
        ..Config::default()
    };
    let response = build_upstream_client(&config)
        .unwrap()
        .get("http://127.0.0.1:18080/packages/item")
        .send()
        .await
        .unwrap();

    assert_eq!(response.text().await.unwrap(), "ok");
    let request = proxy_task.await.unwrap();
    assert!(request.starts_with("GET http://127.0.0.1:18080/packages/item HTTP/1.1\r\n"));
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("proxy-user:{proxy_password}"));
    assert!(request
        .to_ascii_lowercase()
        .contains(&format!("proxy-authorization: basic {}", encoded).to_ascii_lowercase()));
}

#[test]
fn builds_clients_for_every_supported_global_proxy_scheme() {
    for url in [
        "http://127.0.0.1:8080",
        "https://127.0.0.1:8443",
        "socks5://127.0.0.1:1080",
        "socks5h://127.0.0.1:1080",
    ] {
        let config = Config {
            outbound_proxy: OutboundProxyConfig {
                enabled: true,
                url: url.to_string(),
                ..OutboundProxyConfig::default()
            },
            ..Config::default()
        };
        assert!(build_upstream_client(&config).is_ok(), "proxy URL {url}");
    }
}

#[test]
fn upstream_client_loads_an_additional_pem_ca_bundle() {
    const PEM_CA: &[u8] = br#"-----BEGIN CERTIFICATE-----
MIIBtjCCAVugAwIBAgITBmyf1XSXNmY/Owua2eiedgPySjAKBggqhkjOPQQDAjA5
MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g
Um9vdCBDQSAzMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG
A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg
Q0EgMzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCmXp8ZBf8ANm+gBG1bG8lKl
ui2yEujSLtf6ycXYqm0fc4E7O5hrOXwzpcVOho6AF2hiRVd9RFgdszflZwjrZt6j
QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSr
ttvXBp43rDCGB5Fwx5zEGbF4wDAKBggqhkjOPQQDAgNJADBGAiEA4IWSoxe3jfkr
BqWTrBqYaGFy+uGh0PsceGCmQ5nFuMQCIQCcAu/xlJyzlvnrxir4tiz+OpAUFteM
YyRIHN8wfdVoOw==
-----END CERTIFICATE-----
"#;
    let path = std::env::temp_dir().join(format!(
        "mirrorproxy-upstream-ca-{}.pem",
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, PEM_CA).unwrap();
    let config = Config {
        upstream_tls: config::UpstreamTlsConfig {
            ca_certificates: vec![path.to_string_lossy().into_owned()],
            insecure_skip_verify: false,
        },
        ..Config::default()
    };

    let result = build_upstream_client(&config);
    fs::remove_file(path).unwrap();
    assert!(result.is_ok());
}

#[test]
fn control_plane_client_ignores_upstream_tls_overrides() {
    let config = Config {
        upstream_tls: config::UpstreamTlsConfig {
            ca_certificates: vec!["/definitely/missing/mirrorproxy-upstream-ca.pem".to_string()],
            insecure_skip_verify: true,
        },
        ..Config::default()
    };

    assert!(build_upstream_client(&config).is_err());
    assert!(build_control_plane_client(&config).is_ok());
}

#[tokio::test]
async fn global_http_proxy_receives_https_connect_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_headers(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        request
    });
    let config = Config {
        outbound_proxy: OutboundProxyConfig {
            enabled: true,
            url: format!("http://{address}"),
            ..OutboundProxyConfig::default()
        },
        ..Config::default()
    };

    let _ = build_upstream_client(&config)
        .unwrap()
        .get("https://upstream.invalid/archive.tar.zst")
        .send()
        .await;
    let request = proxy_task.await.unwrap();
    assert!(request.starts_with("CONNECT upstream.invalid:443 HTTP/1.1\r\n"));
}

#[tokio::test]
async fn global_proxy_honors_no_proxy_hosts() {
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_address = upstream.local_addr().unwrap();
    let upstream_task = tokio::spawn(async move {
        let (mut stream, _) = upstream.accept().await.unwrap();
        let request = read_http_headers(&mut stream).await;
        respond_ok(&mut stream).await;
        request
    });
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_address = proxy.local_addr().unwrap();
    let config = Config {
        outbound_proxy: OutboundProxyConfig {
            enabled: true,
            url: format!("http://{proxy_address}"),
            no_proxy: vec!["127.0.0.1".to_string()],
            ..OutboundProxyConfig::default()
        },
        ..Config::default()
    };

    let response = build_upstream_client(&config)
        .unwrap()
        .get(format!("http://{upstream_address}/direct"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "ok");
    assert!(upstream_task
        .await
        .unwrap()
        .starts_with("GET /direct HTTP/1.1"));
    assert!(timeout(Duration::from_millis(100), proxy.accept())
        .await
        .is_err());
}

#[tokio::test]
async fn global_socks5h_proxy_resolves_dns_and_authenticates_remotely() {
    let proxy_password = uuid::Uuid::new_v4().to_string();
    let (origin, certificate, tls_handle) = proxy_test_tls_origin("upstream.example.invalid").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 2];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting[0], 5);
        let mut methods = vec![0_u8; greeting[1] as usize];
        stream.read_exact(&mut methods).await.unwrap();
        assert!(methods.contains(&2));
        stream.write_all(&[5, 2]).await.unwrap();

        let mut auth_header = [0_u8; 2];
        stream.read_exact(&mut auth_header).await.unwrap();
        assert_eq!(auth_header[0], 1);
        let mut username = vec![0_u8; auth_header[1] as usize];
        stream.read_exact(&mut username).await.unwrap();
        let password_len = stream.read_u8().await.unwrap();
        let mut password = vec![0_u8; password_len as usize];
        stream.read_exact(&mut password).await.unwrap();
        stream.write_all(&[1, 0]).await.unwrap();

        let mut request_header = [0_u8; 4];
        stream.read_exact(&mut request_header).await.unwrap();
        assert_eq!(request_header, [5, 1, 0, 3]);
        let domain_len = stream.read_u8().await.unwrap();
        let mut domain = vec![0_u8; domain_len as usize];
        stream.read_exact(&mut domain).await.unwrap();
        let port = stream.read_u16().await.unwrap();
        stream
            .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
            .await
            .unwrap();
        let mut upstream = TcpStream::connect(origin).await.unwrap();
        tokio::io::copy_bidirectional(&mut stream, &mut upstream)
            .await
            .unwrap();
        (
            String::from_utf8(username).unwrap(),
            String::from_utf8(password).unwrap(),
            String::from_utf8(domain).unwrap(),
            port,
        )
    });
    let config = Config {
        outbound_proxy: OutboundProxyConfig {
            enabled: true,
            url: format!("socks5h://{address}"),
            username: Some("proxy-user".to_string()),
            password: Some(proxy_password.clone()),
            ..OutboundProxyConfig::default()
        },
        ..Config::default()
    };
    let response = build_upstream_client_builder(&config)
        .unwrap()
        .add_root_certificate(certificate)
        .build()
        .unwrap()
        .get("https://upstream.example.invalid/from-socks")
        .send()
        .await
        .unwrap();

    assert_eq!(response.text().await.unwrap(), "ok");
    // Close any keep-alive connection before awaiting the tunnel.
    tls_handle.shutdown();
    let (username, password, domain, port) = proxy_task.await.unwrap();
    assert_eq!(username, "proxy-user");
    assert_eq!(password, proxy_password);
    assert_eq!(domain, "upstream.example.invalid");
    assert_eq!(port, 443);
}

#[tokio::test]
async fn global_socks5_proxy_resolves_dns_locally() {
    let (origin, certificate, tls_handle) = proxy_test_tls_origin("socks-local.test").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let proxy_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 2];
        stream.read_exact(&mut greeting).await.unwrap();
        let mut methods = vec![0_u8; greeting[1] as usize];
        stream.read_exact(&mut methods).await.unwrap();
        assert!(methods.contains(&0));
        stream.write_all(&[5, 0]).await.unwrap();

        let mut request_header = [0_u8; 4];
        stream.read_exact(&mut request_header).await.unwrap();
        assert_eq!(&request_header[..3], &[5, 1, 0]);
        let address_type = request_header[3];
        match address_type {
            1 => {
                let mut address = [0_u8; 4];
                stream.read_exact(&mut address).await.unwrap();
            }
            4 => {
                let mut address = [0_u8; 16];
                stream.read_exact(&mut address).await.unwrap();
            }
            other => panic!("socks5 local DNS unexpectedly used address type {other}"),
        }
        let port = stream.read_u16().await.unwrap();
        stream
            .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
            .await
            .unwrap();
        let mut upstream = TcpStream::connect(origin).await.unwrap();
        tokio::io::copy_bidirectional(&mut stream, &mut upstream)
            .await
            .unwrap();
        (address_type, port)
    });
    let config = Config {
        outbound_proxy: OutboundProxyConfig {
            enabled: true,
            url: format!("socks5://{address}"),
            ..OutboundProxyConfig::default()
        },
        ..Config::default()
    };
    let response = build_upstream_client_builder(&config)
        .unwrap()
        .add_root_certificate(certificate)
        .resolve(
            "socks-local.test",
            SocketAddr::from(([127, 0, 0, 1], 18080)),
        )
        .build()
        .unwrap()
        .get("https://socks-local.test:18080/from-socks")
        .send()
        .await
        .unwrap();

    assert_eq!(response.text().await.unwrap(), "ok");
    tls_handle.shutdown();
    let (address_type, port) = proxy_task.await.unwrap();
    assert!(matches!(address_type, 1 | 4));
    assert_eq!(port, 18080);
}

#[test]
fn config_value_reads_effective_config_keys() {
    let config = Config::default();

    assert_eq!(
        config_value(&config, "database_path").unwrap(),
        "mirrorproxy.sqlite3"
    );
    assert_eq!(config_value(&config, "public_base_url").unwrap(), "");
    assert_eq!(config_value(&config, "quota.monthly_gb").unwrap(), "500");
    assert_eq!(
        config_value(&config, "quota.bidirectional_accounting").unwrap(),
        "false"
    );
    assert_eq!(config_value(&config, "cache.max_entry_mb").unwrap(), "8");
    assert_eq!(
        config_value(&config, "outbound_proxy.enabled").unwrap(),
        "false"
    );
    assert_eq!(
        config_value(&config, "upstreams.npm").unwrap(),
        "https://registry.npmjs.org"
    );
    assert_eq!(
        config_value(&config, "upstreams.nvm").unwrap(),
        "https://nodejs.org/dist"
    );
    assert_eq!(
        config_value(&config, "upstreams.opam").unwrap(),
        "https://opam.ocaml.org"
    );
    assert_eq!(
        config_value(&config, "upstreams.julia").unwrap(),
        "https://pkg.julialang.org"
    );
    assert_eq!(
        config_value(&config, "upstreams.additional_os.kali").unwrap(),
        "https://kali.download/kali"
    );
    assert_eq!(
        config_value(&config, "upstreams.maven").unwrap(),
        "https://maven-central.storage-download.googleapis.com/maven2"
    );
    assert_eq!(
        config_value(&config, "upstreams.rubygems").unwrap(),
        "https://rubygems.org"
    );
    assert_eq!(
        config_value(&config, "upstreams.nuget").unwrap(),
        "https://api.nuget.org"
    );
    assert_eq!(
        config_value(&config, "upstreams.cpan").unwrap(),
        "https://cpan.metacpan.org"
    );
    assert!(config_value(&config, "missing.key").is_none());
}

#[test]
fn derives_public_base_url_from_request_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("internal:3000"));
    headers.insert(
        "x-forwarded-host",
        HeaderValue::from_static("mirror.example:8443, internal:3000"),
    );
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    assert_eq!(
        request_public_base_url(&headers).as_deref(),
        Some("https://mirror.example:8443")
    );
}

#[test]
fn rejects_invalid_request_public_base_url() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("mirror.example/path"));

    assert!(request_public_base_url(&headers).is_none());
}

#[test]
fn config_entries_include_public_and_quota_settings() {
    let config = Config::default();
    let entries = config_entries(&config);

    assert!(entries
        .iter()
        .any(|(key, value)| key == "enabled_proxies" && value.contains("github")));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "quota.on_exceeded" && value == "stop_proxy"));
    assert!(entries
        .iter()
        .any(|(key, value)| { key == "quota.bidirectional_accounting" && value == "false" }));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "cache.directory" && value == "mirrorproxy-cache"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "outbound_proxy.enabled" && value == "false"));
    assert!(entries
        .iter()
        .any(|(key, value)| { key == "upstream_tls.insecure_skip_verify" && value == "false" }));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.pypi_files"
            && value == "https://files.pythonhosted.org"));
    assert!(entries.iter().any(|(key, value)| key == "upstreams.maven"
        && value == "https://maven-central.storage-download.googleapis.com/maven2"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.rubygems" && value == "https://rubygems.org"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.nuget" && value == "https://api.nuget.org"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.cpan" && value == "https://cpan.metacpan.org"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.opam" && value == "https://opam.ocaml.org"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.julia" && value == "https://pkg.julialang.org"));
    assert!(entries
        .iter()
        .any(|(key, value)| key == "upstreams.additional_os.kali"
            && value == "https://kali.download/kali"));
}

#[test]
fn plan_config_set_builds_dry_run_changes() {
    let config = Config::default();
    let change = plan_config_set(&config, "public_base_url", "https://mirror.example").unwrap();

    assert_eq!(change.key, "public_base_url");
    assert_eq!(change.toml_path, "public_base_url");
    assert_eq!(change.current_value, "");
    assert_eq!(change.next_value, "https://mirror.example");
    assert!(plan_config_set(&config, "public_base_url", "").is_ok());

    let upstream =
        plan_config_set(&config, "upstreams.opam", "https://mirror.example/opam").unwrap();
    assert_eq!(upstream.toml_path, "upstreams.opam");
    assert_eq!(upstream.current_value, "https://opam.ocaml.org");

    let os_upstream = plan_config_set(
        &config,
        "upstreams.additional_os.kali",
        "https://mirror.example/kali",
    )
    .unwrap();
    assert_eq!(os_upstream.toml_path, "upstreams.additional_os.kali");

    let maven = plan_config_set(
        &config,
        "upstreams.maven",
        "https://first.example/maven, https://second.example/maven",
    )
    .unwrap();
    assert_eq!(
        maven.current_value,
        "https://maven-central.storage-download.googleapis.com/maven2"
    );

    let outbound_proxy =
        plan_config_set(&config, "outbound_proxy.url", "socks5h://127.0.0.1:1080").unwrap();
    assert_eq!(outbound_proxy.toml_path, "outbound_proxy.url");

    let ca_certificates = plan_config_set(
        &config,
        "upstream_tls.ca_certificates",
        "/etc/mirrorproxy/ca/company.pem",
    )
    .unwrap();
    assert_eq!(ca_certificates.toml_path, "upstream_tls.ca_certificates");
    assert!(plan_config_set(&config, "upstream_tls.insecure_skip_verify", "true").is_ok());
}

#[test]
fn plan_config_set_validates_values() {
    let config = Config::default();

    assert!(plan_config_set(&config, "missing.key", "value").is_err());
    assert!(plan_config_set(&config, "public_base_url", "file:///tmp").is_err());
    assert!(plan_config_set(&config, "quota.enabled", "yes").is_err());
    assert!(plan_config_set(&config, "quota.bidirectional_accounting", "true").is_ok());
    assert!(plan_config_set(&config, "cache.max_entry_mb", "0").is_err());
    assert!(plan_config_set(&config, "quota.on_exceeded", "drop").is_err());
    assert!(plan_config_set(&config, "timeout.request_secs", "0").is_err());
    assert!(plan_config_set(&config, "quota.monthly_gb", "0").is_ok());
    assert!(plan_config_set(
        &config,
        "upstreams.maven",
        "https://repo.example/maven,ftp://invalid.example/maven",
    )
    .is_err());
    assert!(plan_config_set(&config, "upstreams.maven", "").is_err());
    assert!(plan_config_set(&config, "outbound_proxy.enabled", "true").is_err());
    assert!(plan_config_set(&config, "outbound_proxy.url", "ftp://proxy.example:21").is_err());
}

#[test]
fn persist_config_set_updates_toml_and_keeps_backup() {
    let directory =
        std::env::temp_dir().join(format!("mirrorproxy-config-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let config_path = directory.join("config.toml");
    let original = r#"public_base_url = "http://127.0.0.1:3000"

[quota]
enabled = false
monthly_gb = 500
timezone = "local"
on_exceeded = "stop_proxy"
"#;
    fs::write(&config_path, original).unwrap();

    let change = plan_config_set(
        &Config::load(Some(&config_path)).unwrap(),
        "public_base_url",
        "https://mirror.example",
    )
    .unwrap();
    let backup_path = persist_config_set(&config_path, &change).unwrap();

    assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);
    let updated = Config::load(Some(&config_path)).unwrap();
    assert_eq!(updated.public_base_url, "https://mirror.example");

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn persist_config_set_updates_additional_os_upstream() {
    let directory = std::env::temp_dir().join(format!(
        "mirrorproxy-additional-os-config-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let config_path = directory.join("config.toml");
    fs::write(
        &config_path,
        "[upstreams]\nadditional_os = { kali = \"https://http.kali.org/kali\" }\n",
    )
    .unwrap();

    let change = plan_config_set(
        &Config::load(Some(&config_path)).unwrap(),
        "upstreams.additional_os.kali",
        "https://mirror.example/kali",
    )
    .unwrap();
    persist_config_set(&config_path, &change).unwrap();

    let updated = Config::load(Some(&config_path)).unwrap();
    assert_eq!(
        updated.upstreams.additional_os.get("kali").unwrap(),
        "https://mirror.example/kali"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn persist_config_set_keeps_upstream_groups_as_comma_separated_strings() {
    let directory = std::env::temp_dir().join(format!(
        "mirrorproxy-upstream-group-config-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let config_path = directory.join("config.toml");
    fs::write(
        &config_path,
        "[upstreams]\nnpm = \"https://registry.npmjs.org\"\n",
    )
    .unwrap();

    let value = "https://one.example/npm, https://two.example/npm";
    let change = plan_config_set(
        &Config::load(Some(&config_path)).unwrap(),
        "upstreams.npm",
        value,
    )
    .unwrap();
    persist_config_set(&config_path, &change).unwrap();

    let updated = Config::load(Some(&config_path)).unwrap();
    assert_eq!(updated.upstreams.npm, value);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn persist_config_set_writes_global_outbound_proxy_values() {
    let directory = std::env::temp_dir().join(format!(
        "mirrorproxy-outbound-proxy-config-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let config_path = directory.join("config.toml");
    fs::write(&config_path, "[outbound_proxy]\nenabled = false\n").unwrap();

    for (key, value) in [
        ("outbound_proxy.url", "socks5h://127.0.0.1:1080"),
        ("outbound_proxy.no_proxy", "localhost,127.0.0.1"),
        ("outbound_proxy.enabled", "true"),
    ] {
        let config = Config::load(Some(&config_path)).unwrap();
        let change = plan_config_set(&config, key, value).unwrap();
        persist_config_set(&config_path, &change).unwrap();
    }

    let updated = Config::load(Some(&config_path)).unwrap();
    assert!(updated.outbound_proxy.enabled);
    assert_eq!(updated.outbound_proxy.url, "socks5h://127.0.0.1:1080");
    assert_eq!(updated.outbound_proxy.no_proxy, ["localhost", "127.0.0.1"]);
    fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn healthz_returns_ok() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("\"status\":\"ok\""));
}

#[tokio::test]
async fn metrics_exports_normalized_http_request_series() {
    let app = build_router(Config::default()).await.unwrap();
    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz?token=must-not-appear")
                .header(header::AUTHORIZATION, "Bearer must-not-appear")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let mut request = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:41000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains(
        "mirrorproxy_http_requests_total{method=\"GET\",route=\"/healthz\",status=\"200\"} 1"
    ));
    assert!(!body.contains("must-not-appear"));

    let mut remote_request = Request::builder()
        .uri("/metrics")
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::empty())
        .unwrap();
    remote_request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:41001".parse::<SocketAddr>().unwrap(),
    ));
    let remote_response = app.oneshot(remote_request).await.unwrap();
    assert_eq!(remote_response.status(), StatusCode::FORBIDDEN);
}

#[test]
fn route_groups_never_include_request_paths_or_queries() {
    assert_eq!(
        route_group_for_path("/maven/org/private/artifact.jar"),
        "/proxy/maven"
    );
    assert_eq!(
        route_group_for_path("/api/admin/config"),
        "/api/admin/:action"
    );
    assert_eq!(route_group_for_path("/unknown/token-value"), "/static");
}

#[tokio::test]
async fn rate_limit_returns_too_many_requests() {
    let app = build_router(Config {
        rate_limit: crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 1,
        },
        ..Config::default()
    })
    .await
    .unwrap();

    let first = app
        .clone()
        .oneshot(Request::builder().uri("/cpan").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .oneshot(Request::builder().uri("/cpan").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        second
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .unwrap(),
        "60"
    );
}

#[tokio::test]
async fn rate_limit_does_not_block_health_or_other_clients() {
    let app = build_router(Config {
        rate_limit: crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 1,
        },
        ..Config::default()
    })
    .await
    .unwrap();

    let request = |uri: &'static str, peer: &str| {
        let mut request = Request::builder().uri(uri).body(Body::empty()).unwrap();
        request.extensions_mut().insert(ConnectInfo(
            peer.parse::<SocketAddr>().expect("valid test peer"),
        ));
        request
    };
    assert_eq!(
        app.clone()
            .oneshot(request("/cpan", "192.0.2.1:4000"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(request("/cpan", "192.0.2.1:4001"))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        app.clone()
            .oneshot(request("/cpan", "192.0.2.2:4000"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.oneshot(request("/healthz", "192.0.2.1:4002"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn exposes_public_config() {
    let app = build_router(Config::default()).await.unwrap();
    let mut request = Request::builder()
        .uri("/api/public-config")
        .header("host", "mirror.example:8443")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4242".parse::<SocketAddr>().unwrap()));
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["public_base_url"], "https://mirror.example:8443");
    assert_eq!(value["site"]["title"], "MirrorProxy");
    assert_eq!(value["site"]["icon_url"], "/favicon.svg");
    assert_eq!(value["site"]["footer_text"], "");
    assert!(value["site"]["description"]
        .as_str()
        .unwrap()
        .contains("Hackage"));
    assert_eq!(value["site"]["keywords"].as_array().unwrap().len(), 20);
    assert_eq!(value["enabled_proxies"][0], "github");
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "oci"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "npm"));
    assert_eq!(value["registration"]["mode"], "invite_only");
    assert_eq!(value["registration"]["email_login_enabled"], false);
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "go"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "maven"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "rubygems"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "nuget"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "cpan"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "crates"));
    assert!(value["enabled_proxies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|proxy| proxy == "pypi"));
    assert_eq!(value["quota"]["enabled"], false);
    assert_eq!(value["quota"]["monthly_gb"], 500);
}

#[tokio::test]
async fn admin_config_requires_authentication() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_config_update_persists_and_applies_runtime_values() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let initial_config = Config::default();
    database
        .load_or_seed_runtime_config(initial_config.clone())
        .await
        .unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(initial_config))),
        database: Arc::new(database.clone()),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    let mut next_config = (*state.config()).clone();
    next_config.public_base_url = "https://mirror.example".to_string();
    next_config.enabled_proxies = vec!["npm".to_string()];
    next_config.outbound_proxy = OutboundProxyConfig {
        enabled: true,
        url: "socks5h://proxy.example:1080".to_string(),
        no_proxy: vec!["localhost".to_string()],
        username: Some("proxy-user".to_string()),
        password: Some("proxy-password".to_string()),
    };
    next_config.upstream_tls.insecure_skip_verify = true;
    next_config.alerts.enabled = true;
    next_config.alerts.webhook_url = "https://hooks.example/private-token".to_string();

    let response = update_admin_config(headers, State(state.clone()), Json(next_config)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let response: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        response["config"]["outbound_proxy"]["password"],
        serde_json::Value::Null
    );
    assert_eq!(response["config"]["outbound_proxy"]["has_password"], true);
    assert_eq!(response["config"]["alerts"]["webhook_url"], "");
    assert_eq!(response["config"]["alerts"]["has_webhook_url"], true);
    assert_eq!(
        response["config"]["upstream_tls"]["insecure_skip_verify"],
        true
    );
    assert_eq!(state.config().public_base_url, "https://mirror.example");
    assert_eq!(state.config().enabled_proxies, ["npm"]);

    let reloaded = database
        .load_or_seed_runtime_config(Config::default())
        .await
        .unwrap();
    assert_eq!(reloaded.public_base_url, "https://mirror.example");
    assert_eq!(reloaded.enabled_proxies, ["npm"]);
    assert_eq!(
        reloaded.outbound_proxy.password.as_deref(),
        Some("proxy-password")
    );
    assert!(reloaded.upstream_tls.insecure_skip_verify);
}

#[tokio::test]
async fn super_admin_updates_acme_settings_without_exposing_or_erasing_secrets() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let acme_settings = AcmeConfig {
        enabled: true,
        email: "admin@example.com".to_string(),
        domains: vec!["example.com".to_string(), "*.example.com".to_string()],
        challenge: "dns-01".to_string(),
        dns: config::AcmeDnsConfig {
            provider: "cloudflare".to_string(),
            cloudflare_zone_id: "zone-id".to_string(),
            cloudflare_api_token: "secret-token".to_string(),
            ..config::AcmeDnsConfig::default()
        },
        ..AcmeConfig::default()
    };
    database
        .save_acme_settings("system", &acme_settings)
        .await
        .unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(Config {
            acme: acme_settings.clone(),
            ..Config::default()
        }))),
        database: Arc::new(database.clone()),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );

    let response = admin_acme_config(headers.clone(), State(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["config"]["dns"]["has_cloudflare_api_token"], true);
    assert!(value["config"]["dns"].get("cloudflare_api_token").is_none());
    assert!(!String::from_utf8_lossy(&body).contains("secret-token"));

    let mut update = acme_settings;
    update.domains.push("mirror.example.com".to_string());
    update.dns.cloudflare_api_token.clear();
    let response = update_admin_acme_config(headers, State(state.clone()), Json(update)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["restart_required"], true);
    assert_eq!(
        database
            .acme_settings()
            .await
            .unwrap()
            .unwrap()
            .dns
            .cloudflare_api_token,
        "secret-token"
    );
    assert_eq!(state.config().acme.domains.len(), 3);
}

#[tokio::test]
async fn admin_stats_returns_monthly_usage_and_targets() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let config = Config::default();
    database
        .load_or_seed_runtime_config(config.clone())
        .await
        .unwrap();
    let (day, month) = quota_period(&config.quota.timezone);
    database
        .record_proxy_response(ProxyTrafficRecord {
            day: &day,
            month: &month,
            target_code: "npm",
            method: "GET",
            path: "/npm/react",
            status_code: 200,
            response_bytes: 256,
            delivered_response_bytes: 256,
            stream_error: false,
            reserved_bytes: 0,
            user_id: None,
            group_id: None,
            request_event_retention_days: 30,
            location: &GeoLocation::default(),
        })
        .await
        .unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(config))),
        database: Arc::new(database.clone()),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );

    let response = admin_stats(headers, State(state)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["month"], month);
    assert_eq!(value["response_bytes"], 256);
    assert_eq!(value["targets"][0]["target_code"], "npm");
}

#[tokio::test]
async fn admin_audit_log_requires_authentication_and_returns_entries() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    database
        .save_runtime_config("admin", &Config::default(), "update runtime configuration")
        .await
        .unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(Config::default()))),
        database: Arc::new(database.clone()),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };

    let unauthenticated = admin_audit_log(
        HeaderMap::new(),
        State(state.clone()),
        Query(AuditLogQuery {
            page: 1,
            per_page: 20,
        }),
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    let response = admin_audit_log(
        headers,
        State(state),
        Query(AuditLogQuery {
            page: 1,
            per_page: 20,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| { entry["username"] == "admin" && entry["detail"] == "runtime_config" }));
}

#[tokio::test]
async fn admin_password_change_revokes_current_session() {
    let (database, credentials) = Database::open(":memory:").await.unwrap();
    let credentials = credentials.unwrap();
    let state = AppState {
        downloads: Arc::new(crate::download::Runtime::default()),
        config: Arc::new(RwLock::new(Arc::new(Config::default()))),
        database: Arc::new(database.clone()),
        client: Arc::new(RwLock::new(Client::new())),
        rate_limiter: Arc::new(RateLimiter::new()),
        admin_login_limiter: Arc::new(AdminLoginRateLimiter::new()),
        discoverable_challenges: Default::default(),
        webauthn: Arc::new(RwLock::new(None)),
        observability: Arc::new(Observability::new().unwrap()),
        geoip: Arc::new(GeoIpService::new(
            false,
            "missing-v4.xdb".into(),
            "missing-v6.xdb".into(),
        )),
        ip_access_policy: Arc::new(RwLock::new(IpAccessPolicy::default())),
        acme: test_acme_manager(),
        acme_environment_managed: false,
        upstream_selector: Arc::new(upstream_selection::UpstreamSelector::default()),
    };
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );

    let username = credentials.username.clone();
    let new_password = uuid::Uuid::new_v4().to_string();
    let response = change_admin_password(
        headers,
        State(state),
        Json(AdminPasswordChangeRequest {
            current_password: credentials.password,
            new_password: new_password.clone(),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!database.authorize(&session.token).await.unwrap());
    assert!(database
        .login(&username, &new_password)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn admin_username_change_renames_account_and_revokes_current_session() {
    let (state, credentials) = admin_test_state().await;
    let database = state.database.clone();
    let session = database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );

    let response = change_admin_username(
        headers,
        State(state.clone()),
        Json(AdminUsernameChangeRequest {
            current_password: credentials.password.clone(),
            new_username: "renamed-admin".to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!database.authorize(&session.token).await.unwrap());
    assert!(database
        .login("admin", &credentials.password)
        .await
        .unwrap()
        .is_none());
    assert!(database
        .login("renamed-admin", &credentials.password)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        state.config.read().unwrap().webauthn.break_glass_username,
        "renamed-admin"
    );
}

#[tokio::test]
async fn admin_cookie_login_sets_a_strict_path_scoped_session() {
    let (state, credentials) = admin_test_state().await;
    let response = admin_cookie_login(
        State(state.clone()),
        ConnectInfo("127.0.0.1:41000".parse().unwrap()),
        HeaderMap::new(),
        Json(AdminLoginRequest {
            username: credentials.username.clone(),
            password: credentials.password,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set_cookie.starts_with("mirrorproxy_admin_session="));
    assert!(set_cookie.contains("Path=/admin"));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Secure"));
    assert!(set_cookie.contains("SameSite=Strict"));

    let cookie_pair = set_cookie.split(';').next().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, cookie_pair.parse().unwrap());
    let response = admin_session(headers, State(state)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["username"], credentials.username);
    assert_eq!(value["role"], "super_admin");
}

#[test]
fn user_cookie_is_host_only_lax_and_never_scoped_to_wildcard_subdomains() {
    let cookie = user_session_cookie("test-token");
    let cookie = cookie.to_str().unwrap();
    assert!(cookie.starts_with("mirrorproxy_user_session=test-token"));
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("Domain="));
}

#[tokio::test]
async fn administrator_login_is_limited_by_username_and_source() {
    let (state, credentials) = admin_test_state().await;
    let wrong_password = uuid::Uuid::new_v4().to_string();
    for attempt in 0..5 {
        let response = admin_cookie_login(
            State(state.clone()),
            ConnectInfo("192.0.2.10:41000".parse().unwrap()),
            HeaderMap::new(),
            Json(AdminLoginRequest {
                username: credentials.username.clone(),
                password: wrong_password.clone(),
            }),
        )
        .await;
        if attempt < 4 {
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        } else {
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        }
    }
    let response = admin_cookie_login(
        State(state),
        ConnectInfo("192.0.2.10:41001".parse().unwrap()),
        HeaderMap::new(),
        Json(AdminLoginRequest {
            username: credentials.username,
            password: uuid::Uuid::new_v4().to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::RETRY_AFTER], "900");
}

#[tokio::test]
async fn successful_administrator_logins_do_not_consume_failure_limit() {
    let (state, credentials) = admin_test_state().await;
    for port in 41000..41006 {
        let response = admin_cookie_login(
            State(state.clone()),
            ConnectInfo(format!("192.0.2.11:{port}").parse().unwrap()),
            HeaderMap::new(),
            Json(AdminLoginRequest {
                username: credentials.username.clone(),
                password: credentials.password.clone(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn passkey_only_policy_blocks_password_except_for_break_glass_admin() {
    let (mut state, credentials) = admin_test_state().await;
    let mut config = Config::default();
    config.webauthn.enabled = true;
    config.webauthn.require_passkey = true;
    config.webauthn.break_glass_username = "recovery".to_string();
    state.config = Arc::new(RwLock::new(Arc::new(config)));

    let blocked = admin_cookie_login(
        State(state.clone()),
        ConnectInfo("192.0.2.12:41000".parse().unwrap()),
        HeaderMap::new(),
        Json(AdminLoginRequest {
            username: credentials.username.clone(),
            password: credentials.password.clone(),
        }),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);

    Arc::make_mut(&mut state.config.write().unwrap())
        .webauthn
        .break_glass_username = credentials.username.clone();
    let recovery = admin_cookie_login(
        State(state),
        ConnectInfo("192.0.2.12:41001".parse().unwrap()),
        HeaderMap::new(),
        Json(AdminLoginRequest {
            username: credentials.username,
            password: credentials.password,
        }),
    )
    .await;
    assert_eq!(recovery.status(), StatusCode::OK);
}

#[tokio::test]
async fn passkey_only_policy_rejects_new_administrator_accounts() {
    let (mut state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut config = Config::default();
    config.webauthn.enabled = true;
    config.webauthn.require_passkey = true;
    state.config = Arc::new(RwLock::new(Arc::new(config)));
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{ADMIN_SESSION_COOKIE}={}", session.token)
            .parse()
            .unwrap(),
    );

    let response = create_admin(
        headers,
        State(state.clone()),
        Json(CreateAdminRequest {
            username: "operator".to_string(),
            password: uuid::Uuid::new_v4().to_string(),
            role: "admin".to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(state.database.list_admins().await.unwrap().len(), 1);
}

#[tokio::test]
async fn super_admin_can_create_and_manage_a_routed_user() {
    let (state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{ADMIN_SESSION_COOKIE}={}", session.token)
            .parse()
            .unwrap(),
    );
    let response = create_user(
        headers.clone(),
        State(state.clone()),
        Json(CreateUserRequest {
            email: "person@example.com".to_string(),
            display_name: "Person".to_string(),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(user["routing_id"].as_str().unwrap().len() >= 12);

    let response = list_users(headers, State(state)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let users: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(users.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn passwordless_email_login_creates_user_and_host_only_session() {
    let (mut state, _) = admin_test_state().await;
    let config = Config {
        public_base_url: "https://mirror.example.com".to_string(),
        registration: config::RegistrationConfig {
            mode: "open".to_string(),
            ..config::RegistrationConfig::default()
        },
        ..Config::default()
    };
    state.config = Arc::new(RwLock::new(Arc::new(config)));
    state
        .database
        .save_smtp_settings(
            "admin",
            &database::SmtpSettings {
                enabled: true,
                host: "smtp.example.com".to_string(),
                port: 587,
                security: "starttls".to_string(),
                username: None,
                password: None,
                from_name: "MirrorProxy".to_string(),
                from_address: "mirror@example.com".to_string(),
            },
            false,
        )
        .await
        .unwrap();
    let response = email::request_email_login(
        HeaderMap::new(),
        State(state.clone()),
        ConnectInfo("192.0.2.30:42000".parse().unwrap()),
        Json(email::RequestEmailLogin {
            email: "person+tag@example.com".to_string(),
            invitation_token: None,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let queued = state.database.pending_outbox(1).await.unwrap().remove(0);
    let body = queued.body;
    assert!(body.contains("email=person%2Btag%40example.com"));
    let code = body
        .split("code is ")
        .nth(1)
        .unwrap()
        .chars()
        .take(6)
        .collect::<String>();
    let response = email::verify_email_login(
        State(state.clone()),
        Json(email::VerifyEmailLogin {
            email: "person+tag@example.com".to_string(),
            code: Some(code.clone()),
            token: None,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!cookie.contains("Domain="));
    let repeated = email::verify_email_login(
        State(state),
        Json(email::VerifyEmailLogin {
            email: "person+tag@example.com".to_string(),
            code: Some(code),
            token: None,
        }),
    )
    .await;
    assert_eq!(repeated.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invitation_login_is_bound_to_the_invited_email() {
    let (mut state, _) = admin_test_state().await;
    let config = Config {
        public_base_url: "https://mirror.example.com".to_string(),
        registration: config::RegistrationConfig {
            mode: "invite_only".to_string(),
            ..config::RegistrationConfig::default()
        },
        ..Config::default()
    };
    state.config = Arc::new(RwLock::new(Arc::new(config)));
    state
        .database
        .save_smtp_settings(
            "admin",
            &database::SmtpSettings {
                enabled: true,
                host: "smtp.example.com".to_string(),
                port: 587,
                security: "starttls".to_string(),
                username: None,
                password: None,
                from_name: "MirrorProxy".to_string(),
                from_address: "mirror@example.com".to_string(),
            },
            false,
        )
        .await
        .unwrap();
    let invitation_id = state
        .database
        .create_email_invitation(
            "admin",
            "invited@example.com",
            "Invited User",
            "invitation-token",
            Utc::now().timestamp() + 600,
        )
        .await
        .unwrap();

    let response = email::request_email_login(
        HeaderMap::new(),
        State(state.clone()),
        ConnectInfo("192.0.2.31:42000".parse().unwrap()),
        Json(email::RequestEmailLogin {
            email: "other@example.com".to_string(),
            invitation_token: Some("invitation-token".to_string()),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(state.database.pending_outbox(10).await.unwrap().is_empty());

    let response = email::verify_email_login(
        State(state.clone()),
        Json(email::VerifyEmailLogin {
            email: "invited@example.com".to_string(),
            code: None,
            token: Some("invitation-token".to_string()),
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key(header::SET_COOKIE));
    let user = state
        .database
        .user_by_email("invited@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.display_name, "Invited User");
    let invitation = state
        .database
        .email_invitation(invitation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invitation.status, "accepted");

    let repeated = email::verify_email_login(
        State(state),
        Json(email::VerifyEmailLogin {
            email: "invited@example.com".to_string(),
            code: None,
            token: Some("invitation-token".to_string()),
        }),
    )
    .await;
    assert_eq!(repeated.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn registration_links_use_validated_request_origin_without_public_config() {
    let (state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let app = Router::new()
        .route(
            "/admin/api/registration-invitations",
            post(local_accounts::invite),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            strip_untrusted_forwarded_headers,
        ))
        .with_state(state.clone());
    for (host, peer, forwarded_host, forwarded_proto, native_tls, expected) in [
        (
            "127.0.0.1:3000",
            "127.0.0.1:1234",
            None,
            None,
            false,
            "http://127.0.0.1:3000/login",
        ),
        (
            "[::1]:3000",
            "127.0.0.1:1234",
            None,
            None,
            false,
            "http://[::1]:3000/login",
        ),
        (
            "backend.internal:3000",
            "127.0.0.1:1234",
            Some("mirror.example:8443"),
            Some("https"),
            false,
            "https://mirror.example:8443/login",
        ),
        (
            "127.0.0.1:3000",
            "198.51.100.1:1234",
            Some("attacker.example"),
            Some("https"),
            false,
            "http://127.0.0.1:3000/login",
        ),
        (
            "mirror.example:8443",
            "198.51.100.1:1234",
            None,
            None,
            true,
            "https://mirror.example:8443/login",
        ),
    ] {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/admin/api/registration-invitations")
            .header(header::HOST, host)
            .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(value) = forwarded_host {
            builder = builder.header("x-forwarded-host", value);
        }
        if let Some(value) = forwarded_proto {
            builder = builder.header("x-forwarded-proto", value);
        }
        let mut request = builder
            .body(Body::from(r#"{"billing":{},"expires_in_hours":72}"#))
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        if native_tls {
            request.extensions_mut().insert(HttpsTransport);
        }
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let url = Url::parse(body["url"].as_str().unwrap()).unwrap();
        let token = url.fragment().unwrap().strip_prefix("register=").unwrap();
        assert_eq!(token.len(), 48);
        let mut origin = url.clone();
        origin.set_fragment(None);
        assert_eq!(origin.as_str(), expected);
    }
    Arc::make_mut(&mut state.config.write().unwrap()).public_base_url =
        "https://configured.example".into();
    let request = Request::builder()
        .method("POST")
        .uri("/admin/api/registration-invitations")
        .header(header::HOST, "127.0.0.1:3000")
        .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"billing":{},"expires_in_hours":72}"#))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["url"]
        .as_str()
        .unwrap()
        .starts_with("https://configured.example/login#register="));
    Arc::make_mut(&mut state.config.write().unwrap())
        .public_base_url
        .clear();
    for host in [
        "user:secret@mirror.example",
        "mirror.example/path",
        "mirror.example?query",
    ] {
        let before = state
            .database
            .registration_invitations()
            .await
            .unwrap()
            .len();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/registration-invitations")
                    .header(header::HOST, host)
                    .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"billing":{},"expires_in_hours":72}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            state
                .database
                .registration_invitations()
                .await
                .unwrap()
                .len(),
            before
        );
    }
}

#[tokio::test]
async fn invitation_uses_request_origin_when_public_url_is_empty() {
    let (mut state, credentials) = admin_test_state().await;
    let mut config = Config::default();
    config.public_base_url.clear();
    state.config = Arc::new(RwLock::new(Arc::new(config)));
    state
        .database
        .save_smtp_settings(
            "admin",
            &database::SmtpSettings {
                enabled: true,
                host: "smtp.example.com".to_string(),
                port: 587,
                security: "starttls".to_string(),
                username: None,
                password: None,
                from_name: "MirrorProxy".to_string(),
                from_address: "mirror@example.com".to_string(),
            },
            false,
        )
        .await
        .unwrap();
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:3000"));
    headers.insert(
        header::COOKIE,
        format!("{ADMIN_SESSION_COOKIE}={}", session.token)
            .parse()
            .unwrap(),
    );

    let response = email::create_invitation(
        headers,
        State(state.clone()),
        Json(email::CreateInvitationRequest {
            email: "local@example.com".to_string(),
            display_name: "Local User".to_string(),
            expires_in_hours: 72,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let queued = state.database.pending_outbox(1).await.unwrap().remove(0);
    assert!(queued
        .body
        .contains("http://127.0.0.1:3000/login?email=local%40example.com&token="));
    let html = queued
        .html_body
        .expect("invitation email should include HTML");
    assert!(
        html.contains("<a href=\"http://127.0.0.1:3000/login?email=local%40example.com&amp;token=")
    );
    assert!(html.contains("This one-time invitation expires at"));
}

#[tokio::test]
async fn passkey_only_policy_requires_two_credentials_for_each_non_break_glass_admin() {
    let (state, credentials) = admin_test_state().await;
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{ADMIN_SESSION_COOKIE}={}", session.token)
            .parse()
            .unwrap(),
    );
    let mut config = (*state.config()).clone();
    config.webauthn.enabled = true;
    config.webauthn.require_passkey = true;
    config.webauthn.rp_id = "mirror.example".to_string();
    config.webauthn.rp_origin = "https://mirror.example".to_string();
    config.webauthn.break_glass_username = "recovery".to_string();

    let response = update_admin_config(headers, State(state), Json(config)).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["error"].as_str().unwrap().contains("admin"));
}

#[tokio::test]
async fn passkey_registration_challenge_is_server_stored_session_bound_and_one_time() {
    let (mut state, credentials) = admin_test_state().await;
    let mut config = Config::default();
    config.webauthn.enabled = true;
    config.webauthn.rp_id = "mirror.example".to_string();
    config.webauthn.rp_origin = "https://mirror.example".to_string();
    let webauthn = build_webauthn(&config).unwrap();
    state.config = Arc::new(RwLock::new(Arc::new(config)));
    state.webauthn = Arc::new(RwLock::new(webauthn));
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        format!("{ADMIN_SESSION_COOKIE}={}", session.token)
            .parse()
            .unwrap(),
    );
    let response = start_admin_passkey_registration(headers, State(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["options"]["publicKey"]["rp"]["id"], "mirror.example");
    assert_eq!(
        value["options"]["publicKey"]["authenticatorSelection"]["residentKey"],
        "preferred"
    );
    let challenge_id = value["challenge_id"].as_str().unwrap();
    let stored = state
        .database
        .take_webauthn_challenge(challenge_id, "registration", Some(&session.token))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.0, credentials.username);
    serde_json::from_str::<PasskeyRegistration>(&stored.1).unwrap();
    assert!(state
        .database
        .take_webauthn_challenge(challenge_id, "registration", Some(&session.token))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn discoverable_passkey_start_is_anonymous_one_time_expiring_and_bounded() {
    let (mut state, _) = admin_test_state().await;
    let mut config = Config::default();
    config.webauthn.enabled = true;
    config.webauthn.rp_id = "mirror.example".into();
    config.webauthn.rp_origin = "https://mirror.example".into();
    state.webauthn = Arc::new(RwLock::new(build_webauthn(&config).unwrap()));
    let request = serde_json::from_str::<StartAdminPasskeyLoginRequest>("{}").unwrap();
    let response = start_admin_passkey_login(State(state.clone()), Json(request)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let options = &value["options"]["publicKey"];
    assert_eq!(options["userVerification"], "required");
    assert!(
        options["allowCredentials"].is_null()
            || options["allowCredentials"].as_array().unwrap().is_empty()
    );
    assert!(value["options"]["mediation"].is_null());
    assert!(!String::from_utf8(body.to_vec())
        .unwrap()
        .contains("username"));
    let id = value["challenge_id"].as_str().unwrap();
    let authentication = state.discoverable_challenges.take(id).unwrap();
    assert!(
        state.discoverable_challenges.take(id).is_none(),
        "replay must fail"
    );
    {
        let mut pending = state.discoverable_challenges.pending.lock().unwrap();
        pending.insert(
            "expired".into(),
            (
                Instant::now() - Duration::from_secs(1),
                authentication.clone(),
            ),
        );
    }
    assert!(state.discoverable_challenges.take("expired").is_none());
    for _ in 0..1024 {
        assert!(state
            .discoverable_challenges
            .insert(authentication.clone())
            .is_some());
    }
    assert!(state
        .discoverable_challenges
        .insert(authentication)
        .is_none());
}

#[test]
fn administrator_password_policy_rejects_weak_values() {
    assert!(validate_admin_username("ops.admin").is_ok());
    assert!(validate_admin_username("bad name").is_err());
    let password = uuid::Uuid::new_v4().simple().to_string();
    assert!(validate_admin_password("admin", &password).is_ok());
    assert!(validate_admin_password("admin", &password[..5]).is_err());
    assert!(validate_admin_password(&password, &password).is_err());
    for common in COMMON_ADMIN_PASSWORDS {
        // Exercise policy data, not reusable authentication credentials.
        assert!(validate_admin_password("admin", &common.to_uppercase()).is_err());
    }
}

#[tokio::test]
async fn streamed_proxy_response_records_actual_body_bytes() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let observability = Arc::new(Observability::new().unwrap());
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("hello"))
        .unwrap();
    let traffic = ProxyTraffic::new(
        Arc::new(database.clone()),
        observability.clone(),
        "2026-07-10".to_string(),
        "2026-07".to_string(),
        "npm",
        "GET".to_string(),
        "/npm/react".to_string(),
        1,
        None,
        30,
        GeoLocation::default(),
    )
    .reserve(None, None, 0)
    .await
    .unwrap();
    let response = track_proxy_response(response, traffic);

    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        "hello"
    );
    assert_eq!(
        database
            .traffic_overview("2026-07")
            .await
            .unwrap()
            .response_bytes,
        5
    );
    let (_, metrics) = observability.encode().unwrap();
    assert!(String::from_utf8(metrics)
        .unwrap()
        .contains("mirrorproxy_proxy_response_bytes_total{status=\"200\",target=\"npm\"} 5"));
}

#[tokio::test]
async fn bidirectional_accounting_records_twice_the_streamed_body_bytes() {
    let (database, _) = Database::open(":memory:").await.unwrap();
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from("hello"))
        .unwrap();
    let traffic = ProxyTraffic::new(
        Arc::new(database.clone()),
        Arc::new(Observability::new().unwrap()),
        "2026-07-10".to_string(),
        "2026-07".to_string(),
        "npm",
        "GET".to_string(),
        "/npm/react".to_string(),
        2,
        None,
        30,
        GeoLocation::default(),
    )
    .reserve(None, None, 0)
    .await
    .unwrap();
    let response = track_proxy_response(response, traffic);

    assert_eq!(
        to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        "hello"
    );
    assert_eq!(
        database
            .traffic_overview("2026-07")
            .await
            .unwrap()
            .response_bytes,
        10
    );
}

#[test]
fn quota_period_uses_requested_iana_timezone() {
    let (day, month) = quota_period("Asia/Taipei");
    assert!(day.starts_with(&month));
    assert_eq!(day.len(), 10);
    assert_eq!(month.len(), 7);
}

#[tokio::test]
async fn public_quota_availability_is_uncached_and_matches_real_proxy_rejection() {
    let mut config = Config::default();
    config.quota.enabled = true;
    config.quota.monthly_gb = 1;
    let application = build_application(config).await.unwrap();
    let (_, month) = quota_period(&application.state.config().quota.timezone);
    let limit = 1024 * 1024 * 1024;
    assert!(application
        .state
        .database
        .try_reserve_monthly_bytes(&month, limit, limit)
        .await
        .unwrap());
    for path in ["/api/config", "/api/public-config"] {
        let response = application
            .router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(value["quota"]["status"], "reserved");
        assert_eq!(value["quota"]["month"], month);
        assert!(value["quota"].get("reserved_bytes").is_none());
        assert!(value["quota"].get("response_bytes").is_none());
    }
    let response = application
        .router
        .oneshot(
            Request::builder()
                .uri("/npm/is-number")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(value["scope"], "global");
}

#[tokio::test]
async fn cancelled_proxy_handler_releases_quota_before_response_headers() {
    let mut config = Config::default();
    config.quota.enabled = true;
    config.quota.bidirectional_accounting = true;
    config.quota.monthly_gb = 1;
    let application = build_application(config).await.unwrap();
    let state = application.state;
    let (_, month) = quota_period(&state.config().quota.timezone);
    let started = Arc::new(tokio::sync::Notify::new());
    let handler_started = started.clone();
    let app = Router::new()
        .route(
            "/npm/pending",
            get(move || async move {
                handler_started.notify_one();
                std::future::pending::<Response>().await
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));
    let request = tokio::spawn(
        app.oneshot(
            Request::builder()
                .uri("/npm/pending")
                .body(Body::empty())
                .unwrap(),
        ),
    );
    timeout(Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert!(!state
        .database
        .try_reserve_monthly_bytes(&month, QUOTA_RESERVATION_BYTES * 2, 1)
        .await
        .unwrap());
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    wait_for_cancelled_proxy(&state.database, &month, 0).await;
    assert!(state
        .database
        .try_reserve_monthly_bytes(
            &month,
            QUOTA_RESERVATION_BYTES * 2,
            QUOTA_RESERVATION_BYTES * 2,
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn tcp_disconnect_bills_partial_proxy_body_and_releases_quota() {
    let mut config = Config::default();
    config.quota.enabled = true;
    config.quota.bidirectional_accounting = true;
    config.quota.monthly_gb = 1;
    let application = build_application(config).await.unwrap();
    let state = application.state;
    let (_, month) = quota_period(&state.config().quota.timezone);
    let app = Router::new()
        .route(
            "/npm/stream",
            get(|| async {
                use futures_util::StreamExt;
                Body::from_stream(
                    futures_util::stream::once(async {
                        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"hello"))
                    })
                    .chain(futures_util::stream::pending()),
                )
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET /npm/stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut received = Vec::new();
    timeout(Duration::from_secs(5), async {
        loop {
            let mut buffer = [0; 1024];
            let count = socket.read(&mut buffer).await.unwrap();
            assert_ne!(count, 0);
            received.extend_from_slice(&buffer[..count]);
            if received.windows(5).any(|part| part == b"hello") {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(received.starts_with(b"HTTP/1.1 200"));
    drop(socket);
    wait_for_cancelled_proxy(&state.database, &month, 10).await;
    assert!(state
        .database
        .try_reserve_monthly_bytes(
            &month,
            QUOTA_RESERVATION_BYTES * 2 + 10,
            QUOTA_RESERVATION_BYTES * 2
        )
        .await
        .unwrap());
    server.abort();
    let _ = server.await;
}

async fn wait_for_cancelled_proxy(database: &Database, month: &str, bytes: u64) {
    timeout(Duration::from_secs(5), async {
        loop {
            let usage = database.traffic_overview(month).await.unwrap();
            if usage.request_count != 0 {
                assert_eq!(usage.request_count, 1);
                assert_eq!(usage.response_bytes, bytes);
                assert_eq!(usage.error_count, 1);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled proxy must release quota and record partial usage");
}

#[test]
fn resolves_client_ip_only_through_trusted_proxy_chain() {
    let config = Config {
        trusted_proxies: vec!["127.0.0.1".into(), "10.0.0.0/8".into()],
        ..Config::default()
    };
    let mut request = Request::builder()
        .header("x-forwarded-for", "198.51.100.8, 10.2.0.4")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    assert_eq!(
        resolve_client_ip(&request, &config),
        "198.51.100.8".parse::<IpAddr>().unwrap()
    );

    request.extensions_mut().insert(ConnectInfo(
        "192.0.2.20:4000".parse::<SocketAddr>().unwrap(),
    ));
    assert_eq!(
        resolve_client_ip(&request, &config),
        "192.0.2.20".parse::<IpAddr>().unwrap()
    );
}

#[test]
fn malformed_forwarded_chain_falls_back_to_peer() {
    let config = Config::default();
    let mut request = Request::builder()
        .header("x-forwarded-for", "spoofed-value")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4000".parse::<SocketAddr>().unwrap()));
    assert_eq!(
        resolve_client_ip(&request, &config),
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    );
}

#[test]
fn forwarded_addresses_accept_caddy_remote_with_port() {
    assert_eq!(
        parse_forwarded_ip("198.51.100.8:43120"),
        Some("198.51.100.8".parse().unwrap())
    );
    assert_eq!(
        parse_forwarded_ip("[2001:db8::8]:43120"),
        Some("2001:db8::8".parse().unwrap())
    );
    assert_eq!(parse_forwarded_ip("invalid:43120"), None);
}

#[tokio::test]
async fn quota_guard_blocks_proxy_paths_only() {
    let app = build_router(Config {
        quota: crate::config::QuotaConfig {
            enabled: true,
            monthly_gb: 0,
            ..crate::config::QuotaConfig::default()
        },
        ..Config::default()
    })
    .await
    .unwrap();

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let proxy = app
        .oneshot(Request::builder().uri("/npm/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(proxy.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        proxy
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .unwrap(),
        "3600"
    );
    let body = to_bytes(proxy.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("monthly traffic quota exceeded"));
}

#[tokio::test]
async fn user_subdomain_enforces_personal_quota_when_global_quota_is_disabled() {
    let (mut state, _) = admin_test_state().await;
    let user = state
        .database
        .create_user("admin", "quota@example.com", "Quota User", 12)
        .await
        .unwrap()
        .unwrap();
    state.config = Arc::new(RwLock::new(Arc::new(Config {
        public_base_url: "https://mirror.example.com".to_string(),
        user_access: config::UserAccessConfig {
            base_domain: "mirror.example.com".to_string(),
            ..config::UserAccessConfig::default()
        },
        quota: config::QuotaConfig {
            default_user_monthly_gb: Some(0),
            ..config::QuotaConfig::default()
        },
        ..Config::default()
    })));
    let app = Router::new()
        .route("/npm/{*path}", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            user_routing_middleware,
        ))
        .with_state(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/npm/react")
                .header(
                    header::HOST,
                    format!("{}.mirror.example.com", user.routing_id),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["scope"],
        "user"
    );
}

#[tokio::test]
async fn exposes_source_catalog() {
    let mut config = Config::default();
    config.upstreams.additional_os.insert(
        "clickhouse".to_string(),
        "https://packages.clickhouse.com/deb".to_string(),
    );
    let app = build_router(config).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let capabilities = |code: &str| {
        value["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|target| target["code"] == code)
            .unwrap()["capabilities"]
            .clone()
    };
    let profiles = value["system_profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), catalog::systems::SYSTEM_DISTRIBUTIONS.len());
    for profile in catalog::systems::SYSTEM_DISTRIBUTIONS {
        assert!(profiles.iter().any(|entry| entry["code"] == profile.code
            && entry["versions"] == profile.versions
            && entry["format"] == profile.format.as_str()
            && entry["requires_customer_mirror"] == (profile.code == "rhel")));
    }
    assert_eq!(capabilities("huggingface")["cache"], "bypass");
    assert_eq!(capabilities("huggingface")["authentication"], "public_only");
    assert_eq!(
        capabilities("huggingface")["clients"],
        serde_json::json!(["huggingface_hub SDK", "hf CLI"])
    );
    assert_eq!(capabilities("docker")["cache"], "bypass");
    assert_eq!(capabilities("homebrew")["cache"], "bypass");
    assert_eq!(capabilities("npm")["cache"], "conditional");
    assert_eq!(
        capabilities("clickhouse")["clients"],
        serde_json::json!(["HTTP clients"])
    );

    assert!(value["providers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|provider| provider["code"] == "mirrorproxy"));
    assert!(value["targets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|target| target["code"] == "npm" && target["category"] == "lang"));
    assert!(value["targets"]
        .as_array()
        .unwrap()
        .iter()
        .any(|target| target["code"] == "maven"
            && target["supported_modes"]
                .as_array()
                .is_some_and(|modes| modes
                    .iter()
                    .map(serde_json::Value::as_str)
                    .eq([Some("proxy"), Some("local-config"),]))));
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "npm" && source["provider_code"] == "mirrorproxy"));
    assert!(value["targets"].as_array().unwrap().iter().any(|target| {
        target["code"] == "clickhouse"
            && target["category"] == "repo"
            && target["aliases"] == serde_json::json!(["additional_os"])
            && target["supported_modes"] == serde_json::json!(["proxy"])
    }));
    assert!(value["sources"].as_array().unwrap().iter().any(|source| {
        source["target_code"] == "clickhouse"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/os/clickhouse/"
    }));
    for target_code in ["poetry", "pdm", "uv", "bun"] {
        assert!(value["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["target_code"] == target_code
                && source["provider_code"] == "mirrorproxy"
                && source["capability"] == "proxy"));
    }
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "maven"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/maven/"));
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "rubygems"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/rubygems/"));
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "nuget"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/nuget/v3/index.json"));
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "cpan"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/cpan/"));
    assert!(value["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|source| source["target_code"] == "winget"
            && source["provider_code"] == "mirrorproxy"
            && source["repo_url"] == "/winget/cache"
            && source["capability"] == "proxy"));
    assert!(value["templates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|template| template["target_code"] == "cargo"
            && template["template"]
                .as_str()
                .unwrap()
                .contains("[source.crates-io]")));
    assert!(value["container_registries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|registry| registry["host"] == "mcr.microsoft.com"
            && registry["example_image"] == "mcr.microsoft.com/dotnet/runtime:8.0"));
    for host in [
        "registry.gitlab.com",
        "nvcr.io",
        "container-registry.oracle.com",
    ] {
        assert!(value["container_registries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|registry| registry["host"] == host && registry["legacy"] == false));
    }
}

#[tokio::test]
async fn exposes_unknown_source_health_before_the_first_check() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/source-health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Fixed probes and system roots, including the five supplemental repositories.
    assert_eq!(value["total"], 86);
    assert_eq!(value["unknown"], 86);
    assert_eq!(value["unhealthy"], 0);
    assert_eq!(value["items"].as_array().unwrap().len(), 0);
    assert!(value.get("selection").is_none());
}

#[tokio::test]
async fn selection_details_require_admin_and_stay_out_of_public_health() {
    let (state, credentials) = admin_test_state().await;
    let url = Url::parse("https://upstream.example/private?token=hidden").unwrap();
    state
        .upstream_selector
        .rank(vec![url], &state.config().upstream_selection);
    assert_eq!(
        admin_source_health(HeaderMap::new(), State(state.clone()))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", session.token).parse().unwrap(),
    );
    let response = admin_source_health(headers, State(state.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value["selection"]["samples"][0]["endpoints"][0]["origin"],
        "https://upstream.example"
    );
    assert!(!String::from_utf8(body.to_vec()).unwrap().contains("hidden"));
    let public = public_source_health(State(state)).await;
    let body = to_bytes(public.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value.get("selection").is_none());
}

#[test]
fn source_capabilities_reflect_configuration_without_relaxing_hf_policy() {
    let mut config = Config::default();
    config.cache.enabled = true;
    config.forward_client_authorization = true;
    let npm = SourceCapabilities::for_target("npm", true, vec![], &config);
    assert!(npm.cache_enabled && npm.client_authorization_forwarding);
    let hf = SourceCapabilities::for_target("huggingface", true, vec![], &config);
    assert!(!hf.cache_enabled && !hf.client_authorization_forwarding);
    assert_eq!(hf.authentication, "public_only");
    let guidance = SourceCapabilities::for_target("example", false, vec![], &config);
    assert_eq!(guidance.cache, "not_applicable");
    assert_eq!(guidance.authentication, "not_applicable");
}

#[tokio::test]
async fn oci_root_returns_distribution_ping() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(Request::builder().uri("/v2/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&body[..], b"{}");
}

#[tokio::test]
async fn go_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/goproxy/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Go module proxy"));
}

#[tokio::test]
async fn maven_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/maven/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Maven repository proxy"));
}

#[tokio::test]
async fn rubygems_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/rubygems/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("RubyGems repository proxy"));
}

#[tokio::test]
async fn nuget_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/nuget/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("NuGet v3 repository proxy"));
}

#[tokio::test]
async fn cpan_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/cpan/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("CPAN repository proxy"));
}

#[tokio::test]
async fn guix_root_returns_proxy_info() {
    let app = build_router(Config::default()).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/guix/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("GNU Guix substitute cache"));
}

#[tokio::test]
async fn crates_index_config_points_to_local_downloads() {
    let app = build_router(Config::default()).await.unwrap();
    let mut request = Request::builder()
        .uri("/crates-index/config.json")
        .header("host", "mirror.example")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:4242".parse::<SocketAddr>().unwrap()));
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .unwrap(),
        "public, max-age=300, stale-while-revalidate=3600"
    );
    assert_eq!(
        response.headers().get(axum::http::header::VARY),
        Some(&HeaderValue::from_static(
            "X-Forwarded-Host, X-Forwarded-Proto"
        ))
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["dl"], "https://mirror.example/crates/api/v1/crates");
}

#[tokio::test]
async fn ignores_forwarded_headers_from_an_untrusted_peer() {
    let app = build_router(Config::default()).await.unwrap();
    let mut request = Request::builder()
        .uri("/api/public-config")
        .header("host", "mirror.example")
        .header("x-forwarded-host", "attacker.example")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.10:4242".parse::<SocketAddr>().unwrap(),
    ));

    let response = app.oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["public_base_url"], "http://mirror.example");
}

#[tokio::test]
async fn sqids_subdomain_resolves_user_and_main_domain_obeys_required_mode() {
    let (state, user) = routing_test_state("subdomain_required").await;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::HOST,
        format!("{}.mirror.example.com", user.routing_id)
            .parse()
            .unwrap(),
    );
    assert_eq!(
        state.public_base_url(&headers),
        format!("https://{}.mirror.example.com", user.routing_id)
    );
    let app = Router::new()
        .route(
            "/npm/pkg",
            get(
                |Extension(context): Extension<UserRoutingContext>| async move {
                    format!("{}:{}", context.user_id, context.routing_id)
                },
            ),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            user_routing_middleware,
        ))
        .with_state(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/npm/pkg")
                .header("host", format!("{}.mirror.example.com", user.routing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        String::from_utf8(body.to_vec()).unwrap(),
        format!("{}:{}", user.id, user.routing_id)
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/npm/pkg")
                .header("host", "mirror.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn user_subdomains_reject_control_paths_unknown_ids_and_spoofed_hosts() {
    let (state, user) = routing_test_state("subdomain_required").await;
    let app = Router::new()
        .fallback(|| async { StatusCode::OK })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            user_routing_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            strip_untrusted_forwarded_headers,
        ))
        .with_state(state);

    for (host, path) in [
        (format!("{}.mirror.example.com", user.routing_id), "/admin"),
        ("unknown12345.mirror.example.com".to_string(), "/npm/pkg"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("host", host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    let mut request = Request::builder()
        .uri("/npm/pkg")
        .header("host", "mirror.example.com")
        .header(
            "x-forwarded-host",
            format!("{}.mirror.example.com", user.routing_id),
        )
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "198.51.100.20:42000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn public_access_mode_keeps_main_proxy_paths_and_rejects_foreign_hosts() {
    let (state, _) = routing_test_state("public").await;
    let app = Router::new()
        .fallback(|| async { StatusCode::OK })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            user_routing_middleware,
        ))
        .with_state(state);
    let main = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/npm/pkg")
                .header("host", "mirror.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(main.status(), StatusCode::OK);
    let foreign = app
        .oneshot(
            Request::builder()
                .uri("/npm/pkg")
                .header("host", "other.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn os_and_oci_routes_do_not_fetch_redirected_internal_content() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let internal_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let internal_address = internal_listener.local_addr().unwrap();
    let internal_hits = Arc::new(AtomicUsize::new(0));
    let hits = internal_hits.clone();
    let internal_app = Router::new().fallback(move || {
        hits.fetch_add(1, Ordering::SeqCst);
        async { "internal secret" }
    });
    let internal_server =
        tokio::spawn(async move { axum::serve(internal_listener, internal_app).await.unwrap() });

    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_address = upstream_listener.local_addr().unwrap();
    let location = format!("http://{internal_address}/secret");
    let upstream_app = Router::new().fallback(move || {
        let location = location.clone();
        async move { (StatusCode::FOUND, [(header::LOCATION, location)]) }
    });
    let upstream_server =
        tokio::spawn(async move { axum::serve(upstream_listener, upstream_app).await.unwrap() });

    let mut config = Config::default();
    config.upstreams.debian = format!("http://{upstream_address}");
    config.upstreams.docker_hub = format!("http://{upstream_address}");
    let app = build_router(config).await.unwrap();
    for path in [
        "/os/debian/dists/stable/Release",
        "/v2/nginx/manifests/latest",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{path}");
    }
    assert_eq!(internal_hits.load(Ordering::SeqCst), 0);
    upstream_server.abort();
    internal_server.abort();
}

#[tokio::test]
async fn huggingface_downloads_preserve_metadata_ranges_and_public_only_access() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let seen = hits.clone();
    let upstream = Router::new().fallback(move |request: Request<Body>| {
        let seen = seen.clone();
        async move {
            assert!(!request.headers().contains_key(header::AUTHORIZATION));
            assert!(!request.headers().contains_key(header::COOKIE));
            assert!(!request.headers().contains_key("x-api-key"));
            assert_eq!(request.headers()[header::ACCEPT_ENCODING], "identity");
            seen.fetch_add(1, Ordering::SeqCst);
            if request.uri().path().ends_with("/resolve/main/model.bin") {
                return Response::builder()
                    .status(302)
                    .header(header::LOCATION, "/blob")
                    .header("x-repo-commit", "0123456789012345678901234567890123456789")
                    .header("x-linked-etag", "\"file-hash\"")
                    .header("x-linked-size", "10")
                    .header("x-xet-hash", "must-not-escape")
                    .body(Body::empty())
                    .unwrap();
            }
            if request.headers().contains_key(header::IF_NONE_MATCH) {
                return StatusCode::NOT_MODIFIED.into_response();
            }
            let (status, range, payload) = match request
                .headers()
                .get(header::RANGE)
                .and_then(|v| v.to_str().ok())
            {
                Some("bytes=4-7") => (StatusCode::PARTIAL_CONTENT, Some("bytes 4-7/10"), "4567"),
                Some(_) => (StatusCode::RANGE_NOT_SATISFIABLE, Some("bytes */10"), ""),
                None => (StatusCode::OK, None, "0123456789"),
            };
            let mut response = Response::builder()
                .status(status)
                .header(header::CONTENT_LENGTH, payload.len())
                .header(header::ETAG, "\"file-hash\"")
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::SET_COOKIE, "must-not-escape=value");
            if let Some(range) = range {
                response = response.header(header::CONTENT_RANGE, range);
            }
            response
                .body(if request.method() == Method::HEAD {
                    Body::empty()
                } else {
                    Body::from(payload)
                })
                .unwrap()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let cache_dir = std::env::temp_dir().join(format!("mirrorproxy-hf-{}", uuid::Uuid::new_v4()));
    let mut config = Config {
        forward_client_authorization: true,
        ..Config::default()
    };
    config.geoip.enabled = false;
    config.upstreams.huggingface = format!("http://{address}");
    config.upstream_auth.insert(
        "huggingface".to_string(),
        config::UpstreamAuth {
            username: None,
            password: None,
            bearer_token: Some(uuid::Uuid::new_v4().to_string()),
        },
    );
    config.cache.enabled = true;
    config.cache.directory = cache_dir.to_string_lossy().into_owned();
    config.validate().unwrap();
    assert!(config
        .upstream_auth_for(&Url::parse(&format!("http://{address}/blob")).unwrap())
        .is_some());
    let app = build_router(config).await.unwrap();
    let path = "/huggingface/org/model/resolve/main/model.bin";
    for (method, range, expected_status, expected_body) in [
        ("HEAD", None, StatusCode::OK, ""),
        ("GET", None, StatusCode::OK, "0123456789"),
        ("GET", None, StatusCode::OK, "0123456789"),
        (
            "GET",
            Some("bytes=4-7"),
            StatusCode::PARTIAL_CONTENT,
            "4567",
        ),
        (
            "GET",
            Some("bytes=99-"),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "",
        ),
    ] {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, "Bearer client-secret")
            .header(header::COOKIE, "session=client-secret")
            .header("x-api-key", "client-secret");
        if let Some(range) = range {
            request = request
                .header(header::RANGE, range)
                .header(header::IF_RANGE, "\"file-hash\"");
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        assert_eq!(response.headers()["x-mirrorproxy-cache"], "BYPASS");
        assert!(!response.headers().contains_key(header::LOCATION));
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert!(!response.headers().contains_key("x-xet-hash"));
        if expected_status.is_success() {
            assert_eq!(
                response.headers()["x-repo-commit"],
                "0123456789012345678901234567890123456789"
            );
            assert_eq!(response.headers()["x-linked-size"], "10");
        }
        if range == Some("bytes=4-7") {
            assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 4-7/10");
        }
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body, expected_body);
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        10,
        "each HEAD/GET must reach the origin and blob"
    );
    assert!(
        !cache_dir.exists(),
        "HF must not write the enabled shared cache"
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header(header::IF_NONE_MATCH, "\"file-hash\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    for (method, path, status) in [
        ("POST", path, StatusCode::METHOD_NOT_ALLOWED),
        ("GET", "/huggingface/api/whoami-v2", StatusCode::BAD_REQUEST),
        (
            "GET",
            "/huggingface/gpt2/resolve/main/%252e%252e/file",
            StatusCode::BAD_REQUEST,
        ),
        (
            "GET",
            "/huggingface/gpt2/resolve/main/file?token=secret",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            status
        );
    }
    server.abort();
    let mut config = Config::default();
    config.enabled_proxies.retain(|p| p != "huggingface");
    let app = build_router(config).await.unwrap();
    assert_eq!(
        app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert!(is_proxy_path(path));
    assert_eq!(proxy_target_for_path(path), Some("huggingface"));
}

#[tokio::test]
async fn huggingface_uses_configured_upstream_group_failover() {
    let upstream = Router::new().fallback(|request: Request<Body>| async move {
        if request.uri().path().starts_with("/unavailable/") {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        } else {
            assert!(request.uri().path().starts_with("/hub/gpt2/resolve/"));
            "public file".into_response()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let mut config = Config::default();
    config.upstreams.huggingface = format!("http://{address}/unavailable, http://{address}/hub");
    let app = build_router(config).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/huggingface/gpt2/resolve/main/file")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        "public file"
    );
    server.abort();
}

#[tokio::test]
async fn huggingface_streams_past_total_timeout_and_rejects_unsafe_redirects() {
    let upstream = Router::new().fallback(|request: Request<Body>| async move {
        if request.uri().path().ends_with("/unsafe") {
            return Response::builder()
                .status(302)
                .header(header::LOCATION, "http://169.254.169.254/latest/meta-data")
                .body(Body::empty())
                .unwrap();
        }
        let stream = futures_util::stream::unfold(0, |i| async move {
            if i == 3 {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
            Some((
                Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"chunk")),
                i + 1,
            ))
        });
        Response::builder().body(Body::from_stream(stream)).unwrap()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let mut config = Config::default();
    config.upstreams.huggingface = format!("http://{address}");
    config.timeout.request_secs = 1;
    let app = build_router(config).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/huggingface/gpt2/resolve/main/file")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        "chunkchunkchunk"
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/huggingface/gpt2/resolve/main/unsafe")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    server.abort();
}

#[tokio::test]
async fn os_repository_streams_past_total_timeout() {
    let upstream = Router::new().fallback(|| async move {
        let stream = futures_util::stream::unfold(0, |i| async move {
            if i == 3 {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(600)).await;
            Some((
                Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"chunk")),
                i + 1,
            ))
        });
        Response::builder().body(Body::from_stream(stream)).unwrap()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let mut config = Config::default();
    config.upstreams.alpine = format!("http://{address}");
    config.timeout.request_secs = 1;
    let app = build_router(config).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/os/alpine/v3.21/main/package.apk")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap(),
        "chunkchunkchunk"
    );
    server.abort();
}

#[tokio::test]
async fn pypi_relative_index_links_download_from_packages_with_matching_digest() {
    use sha2::{Digest, Sha256};

    let payload = b"package archive fixture";
    let digest = hex::encode(Sha256::digest(payload));
    for base_path in ["", "/python"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let index_path = format!("{base_path}/simple/demo/");
        let file_path = format!("{base_path}/packages/aa/demo.whl?download=1");
        let html = format!(
            "<a href=\"../../packages/aa/demo.whl?download=1#sha256={digest}\">demo.whl</a>"
        );
        let upstream = Router::new().fallback(move |request: Request<Body>| {
            let html = html.clone();
            let index_path = index_path.clone();
            let file_path = file_path.clone();
            async move {
                if request.uri().path() == index_path {
                    ([(header::CONTENT_TYPE, "text/html")], html).into_response()
                } else if request.uri().to_string() == file_path {
                    (
                        [(header::CONTENT_TYPE, "application/octet-stream")],
                        payload.as_slice(),
                    )
                        .into_response()
                } else {
                    StatusCode::NOT_FOUND.into_response()
                }
            }
        });
        let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
        let mut config = Config {
            public_base_url: "https://mirror.example".to_string(),
            ..Config::default()
        };
        config.upstreams.pypi_simple = format!("http://{address}{base_path}/simple");
        config.upstreams.pypi_files = format!("http://{address}{base_path}");
        let app = build_router(config).await.unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/pypi/simple/demo/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        let link = html
            .split("href=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let url = reqwest::Url::parse(link).unwrap();
        assert_eq!(url.fragment().unwrap(), format!("sha256={digest}"));
        assert_eq!(url.path(), "/pypi/packages/aa/demo.whl");
        for path in [
            format!("{}?{}", url.path(), url.query().unwrap()),
            "/pypi/files/packages/aa/demo.whl?download=1".to_string(),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            assert_eq!(body.as_ref(), payload);
            assert_eq!(hex::encode(Sha256::digest(&body)), digest);
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("HEAD")
                    .uri("/pypi/packages/aa/demo.whl?download=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty());
        server.abort();
    }
}

#[tokio::test]
async fn pypi_packages_respects_enablement_and_proxy_classification() {
    let config = Config {
        enabled_proxies: vec!["npm".to_string()],
        ..Config::default()
    };
    let app = build_router(config).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/pypi/packages/aa/pkg.whl")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        proxy_target_for_path("/pypi/packages/aa/pkg.whl"),
        Some("pypi")
    );
    assert!(is_proxy_path("/pypi/packages/aa/pkg.whl"));
}

#[tokio::test]
async fn pypi_file_path_validation_rejects_traversal() {
    let app = build_router(Config::default()).await.unwrap();
    for path in [
        "/pypi/files/../pkg.whl",
        "/pypi/packages/../pkg.whl",
        "/pypi/packages/%2e%2e/pkg.whl",
        "/pypi/packages/aa%5cbb/pkg.whl",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}

#[tokio::test]
async fn serves_embedded_index() {
    let config = Config {
        public_base_url: "https://mirror.example".to_string(),
        site: config::SiteConfig {
            title: "Mirror & Packages".to_string(),
            description: "Fast <private> mirrors".to_string(),
            keywords: vec!["mirror".to_string(), "packages".to_string()],
            icon_url: "https://cdn.example/icon.png".to_string(),
            footer_text: "Private mirror service".to_string(),
        },
        ..Config::default()
    };
    let app = build_router(config).await.unwrap();
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .unwrap(),
        "no-cache"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("<title>Mirror &amp; Packages</title>"));
    assert!(body.contains("content=\"Fast &lt;private&gt; mirrors\""));
    assert!(body.contains("content=\"mirror, packages\""));
    assert!(body.contains("href=\"https://cdn.example/icon.png\""));
    assert!(body.contains("href=\"https://mirror.example\""));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body)
        .contains("<meta name=\"robots\" content=\"noindex,nofollow\""));
}

#[tokio::test]
async fn user_summary_requires_super_admin_and_preserves_config_snapshots() {
    let (state, credentials) = admin_test_state().await;
    let viewer_password = uuid::Uuid::new_v4().to_string();
    let snapshot = state.config();
    assert!(Arc::ptr_eq(&snapshot, &state.config()));
    Arc::make_mut(&mut state.config.write().unwrap()).site.title = "Updated".into();
    assert_ne!(snapshot.site.title, state.config().site.title);
    let app = Router::new()
        .route(
            "/admin/api/users/summary",
            get(admin_billing::user_summaries),
        )
        .with_state(state.clone());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/users/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    state
        .database
        .create_admin("admin", "viewer", &viewer_password, "admin")
        .await
        .unwrap();
    let viewer = state
        .database
        .login("viewer", &viewer_password)
        .await
        .unwrap()
        .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/api/users/summary")
                .header(header::AUTHORIZATION, format!("Bearer {}", viewer.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let session = state
        .database
        .login(&credentials.username, &credentials.password)
        .await
        .unwrap()
        .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/api/users/summary?page=0&per_page=200")
                .header(header::AUTHORIZATION, format!("Bearer {}", session.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["page"], 1);
    assert_eq!(value["per_page"], 100);
    assert_eq!(value["total"], 0);
}

#[test]
fn monthly_quota_blank_is_unlimited_but_zero_is_a_zero_allowance() {
    assert_eq!(quota_gb_to_bytes(None).unwrap(), None);
    assert_eq!(quota_gb_to_bytes(Some(0)).unwrap(), Some(0));
    assert_eq!(quota_gb_to_bytes(Some(1)).unwrap(), Some(1 << 30));
}
