//! Authenticated public HTTPS downloads. Never reuse the configurable mirror
//! client: proxy DNS, insecure TLS and operator-private origins are forbidden here.
use super::*;
use futures_util::stream;
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DownloadConfig {
    pub enabled: bool,
    pub max_file_bytes: u64,
    pub daily_user_bytes: u64,
    pub max_concurrent: usize,
    pub max_user_concurrent: usize,
    pub requests_per_minute: u32,
    pub max_duration_secs: u64,
    pub bandwidth_bytes_per_sec: u64,
}
impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_file_bytes: 10 << 30,
            daily_user_bytes: 20 << 30,
            max_concurrent: 16,
            max_user_concurrent: 5,
            requests_per_minute: 300,
            max_duration_secs: 1800,
            bandwidth_bytes_per_sec: 100 << 20,
        }
    }
}
impl DownloadConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=64 << 30).contains(&self.max_file_bytes),
            "download.max_file_bytes must be 1 to 64 GiB"
        );
        anyhow::ensure!(
            (1..=i64::MAX as u64 / 2).contains(&self.daily_user_bytes),
            "invalid download.daily_user_bytes"
        );
        anyhow::ensure!(
            (1..=128).contains(&self.max_concurrent)
                && (1..=self.max_concurrent).contains(&self.max_user_concurrent),
            "invalid download concurrency limits"
        );
        anyhow::ensure!(
            (1..=3600).contains(&self.requests_per_minute),
            "invalid download.requests_per_minute"
        );
        anyhow::ensure!(
            (1..=86400).contains(&self.max_duration_secs),
            "invalid download.max_duration_secs"
        );
        anyhow::ensure!(
            (65536..=1 << 30).contains(&self.bandwidth_bytes_per_sec),
            "download bandwidth must be 64 KiB/s to 1 GiB/s"
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct Runtime {
    active: Mutex<HashMap<String, Active>>,
    pace: tokio::sync::Mutex<Option<Instant>>,
}
struct Active {
    user: i64,
    hosts: Vec<String>,
    cancel: watch::Sender<bool>,
}
struct Permit {
    id: String,
    runtime: Arc<Runtime>,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.runtime.active.lock().unwrap().remove(&self.id);
    }
}
impl Runtime {
    fn acquire(
        self: &Arc<Self>,
        user: i64,
        config: &DownloadConfig,
    ) -> Option<(Permit, watch::Receiver<bool>)> {
        let mut active = self.active.lock().unwrap();
        if active.len() >= config.max_concurrent
            || active.values().filter(|a| a.user == user).count() >= config.max_user_concurrent
        {
            return None;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let (cancel, rx) = watch::channel(false);
        active.insert(
            id.clone(),
            Active {
                user,
                hosts: vec![],
                cancel,
            },
        );
        Some((
            Permit {
                id,
                runtime: self.clone(),
            },
            rx,
        ))
    }
    fn host(&self, id: &str, host: &str) {
        if let Some(a) = self.active.lock().unwrap().get_mut(id) {
            a.hosts.push(host.into());
        }
    }
    fn cancel_domain(&self, domain: &str, subdomains: bool) {
        for a in self.active.lock().unwrap().values() {
            if a.hosts
                .iter()
                .any(|h| h == domain || (subdomains && h.ends_with(&format!(".{domain}"))))
            {
                let _ = a.cancel.send(true);
            }
        }
    }
    fn cancel_user(&self, user: i64) {
        for a in self.active.lock().unwrap().values() {
            if a.user == user {
                let _ = a.cancel.send(true);
            }
        }
    }
    pub(crate) fn cancel_all(&self) {
        for a in self.active.lock().unwrap().values() {
            let _ = a.cancel.send(true);
        }
    }
    fn list(&self) -> Value {
        json!(self
            .active
            .lock()
            .unwrap()
            .iter()
            .map(|(id, a)| json!({"id":id,"user_id":a.user,"domains":a.hosts}))
            .collect::<Vec<_>>())
    }
    async fn throttle(&self, bytes: usize, rate: u64) {
        // Reserve time only after the previous waiter completes. Cancellation
        // releases this async lock without leaving abandoned future reservations.
        let mut pace = self.pace.lock().await;
        let now = Instant::now();
        let delay = pace.unwrap_or(now).saturating_duration_since(now);
        tokio::time::sleep(delay).await;
        *pace = Some(Instant::now() + Duration::from_secs_f64(bytes as f64 / rate as f64));
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/download", get(fetch).head(fetch))
        .route("/api/download/check", get(check))
        .route("/api/account/download", get(account))
        .route("/api/account/download/tokens", post(create_token))
        .route("/api/account/download/tokens/{id}", delete(revoke_token))
        .route("/admin/api/download", get(admin))
        .route(
            "/admin/api/download/config",
            axum::routing::put(update_config),
        )
        .route(
            "/admin/api/download/permissions",
            axum::routing::put(permission),
        )
        .route("/admin/api/download/blocks", axum::routing::put(block))
        .route("/admin/api/download/active/{id}", delete(terminate))
}
fn reject(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error":message}))).into_response()
}
fn failure(status: StatusCode, message: &str) -> Box<Response> {
    Box::new(reject(status, message))
}
fn forbidden() -> Response {
    reject(
        StatusCode::FORBIDDEN,
        "download permission required or revoked",
    )
}
async fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(i64, String), Box<Response>> {
    if !state.config().download.enabled {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "custom downloads are disabled",
        ));
    }
    let Some(token) = bearer_token(headers) else {
        return Err(Box::new(unauthorized_response()));
    };
    match state.database.authenticate_download_token(token).await {
        Ok(Some(user)) => Ok((user, token.into())),
        Ok(None) => Err(Box::new(forbidden())),
        Err(_) => Err(Box::new(internal_error_response())),
    }
}

/// Restrict syntax before any DNS or upstream I/O. Literal IPs are unnecessary
/// for artifact downloads and are refused, including alternate IPv4 spellings.
fn target(value: &str) -> anyhow::Result<Url> {
    anyhow::ensure!(
        value.len() <= 8192 && !value.chars().any(char::is_control) && !value.contains('\\'),
        "invalid download URL"
    );
    let url = Url::parse(value).map_err(|_| anyhow::anyhow!("invalid download URL"))?;
    anyhow::ensure!(
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "only credential-free HTTPS on port 443 is allowed"
    );
    let Some(url::Host::Domain(host)) = url.host() else {
        anyhow::bail!("a public DNS hostname is required");
    };
    anyhow::ensure!(valid_domain(host), "invalid public hostname");
    Ok(url)
}
fn valid_domain(host: &str) -> bool {
    host.len() <= 253
        && host.contains('.')
        && !host.ends_with('.')
        && host.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn domain_input(input: &str) -> Option<String> {
    let u = target(&format!("https://{input}/")).ok()?;
    let h = u.host_str()?;
    // Disallow inputs that smuggle a path, port, or query into a domain rule.
    (u.path() == "/" && u.query().is_none() && input.eq_ignore_ascii_case(h)).then(|| h.to_owned())
}
pub(crate) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let n = u32::from(v);
            // IANA special-purpose space, including metadata/CGNAT, benchmarking,
            // documentation, multicast and reserved ranges. Conservatively deny
            // the entire 192.0/16 special block.
            ![
                (0, 8),
                (0x0a000000, 8),
                (0x64400000, 10),
                (0x7f000000, 8),
                (0xa9fe0000, 16),
                (0xa83f8110, 32), // Azure platform virtual IP
                (0xac100000, 12),
                (0xc0000000, 16),
                (0xc0a80000, 16),
                (0xc6120000, 15),
                (0xc6336400, 24),
                (0xcb007100, 24),
                (0xe0000000, 3),
            ]
            .iter()
            .any(|&(net, bits)| n >> (32 - bits) == net >> (32 - bits))
        }
        IpAddr::V6(v) => {
            let n = u128::from(v);
            // Only ordinary global unicast, excluding transition/tunnelling,
            // documentation, protocol assignments and mapped IPv4 addresses.
            n >> 125 == 1
                && ![
                    (0x2001u128 << 112, 23),
                    (0x20010db8u128 << 96, 32),
                    (0x2002u128 << 112, 16),
                    (0x3ff0u128 << 112, 12),
                ]
                .iter()
                .any(|&(net, bits)| n >> (128 - bits) == net >> (128 - bits))
        }
    }
}
async fn resolved(url: &Url) -> anyhow::Result<Vec<SocketAddr>> {
    let host = url.host_str().context("missing hostname")?;
    let addresses =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host, 443)))
            .await
            .context("DNS timeout")?
            .map_err(|_| anyhow::anyhow!("DNS lookup failed"))?
            .collect::<Vec<_>>();
    anyhow::ensure!(
        !addresses.is_empty()
            && addresses.len() <= 64
            && addresses.iter().all(|a| public_ip(a.ip())),
        "target does not resolve exclusively to public addresses"
    );
    Ok(addresses)
}
fn pinned_client(host: &str, addresses: &[SocketAddr]) -> ClientBuilder {
    Client::builder()
        .no_proxy()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, addresses)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(20))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .referer(false)
}
async fn upstream(
    state: &AppState,
    permit: &Permit,
    url: Url,
    method: Method,
    range: Option<&str>,
) -> Result<(reqwest::Response, String, Vec<String>), Box<Response>> {
    let mut url = url;
    let mut hosts = vec![];
    for _ in 0..=5 {
        let host = url.host_str().unwrap().to_owned();
        state.downloads.host(&permit.id, &host);
        hosts.push(host.clone());
        if state
            .database
            .download_blocked(&host)
            .await
            .map_err(|_| Box::new(internal_error_response()))?
        {
            return Err(failure(StatusCode::FORBIDDEN, "download domain is blocked"));
        }
        let addresses = resolved(&url)
            .await
            .map_err(|_| failure(StatusCode::FORBIDDEN, "target DNS/address is not allowed"))?;
        let client = pinned_client(&host, &addresses)
            .build()
            .map_err(|_| Box::new(internal_error_response()))?;
        let mut request = client
            .request(method.clone(), url.clone())
            .header(header::ACCEPT_ENCODING, "identity")
            .header(
                header::USER_AGENT,
                concat!("MirrorProxy-Download/", env!("CARGO_PKG_VERSION")),
            );
        if let Some(range) = range {
            request = request.header(header::RANGE, range);
        }
        // Deliberately do not use the shared mirror forwarding/header helpers.
        let response = request
            .send()
            .await
            .map_err(|_| failure(StatusCode::BAD_GATEWAY, "upstream download request failed"))?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .ok_or_else(|| failure(StatusCode::BAD_GATEWAY, "invalid upstream redirect"))?;
            let next = url
                .join(location)
                .map_err(|_| failure(StatusCode::BAD_GATEWAY, "invalid upstream redirect"))?;
            url = target(next.as_str())
                .map_err(|_| failure(StatusCode::FORBIDDEN, "redirect target is not allowed"))?;
            continue;
        }
        return Ok((response, host, hosts));
    }
    Err(failure(
        StatusCode::BAD_GATEWAY,
        "too many upstream redirects",
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadQuery {
    url: String,
}
#[allow(clippy::result_large_err)]
fn range_header(headers: &HeaderMap) -> Result<Option<String>, Response> {
    let Some(value) = headers.get(header::RANGE) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| reject(StatusCode::BAD_REQUEST, "invalid Range"))?;
    let Some((a, b)) = value.strip_prefix("bytes=").and_then(|v| v.split_once('-')) else {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            "only a single byte range is supported",
        ));
    };
    let start = a.parse::<u64>().ok();
    let end = if b.is_empty() {
        Some(u64::MAX)
    } else {
        b.parse::<u64>().ok()
    };
    if start.zip(end).is_none_or(|(a, b)| a > b) {
        return Err(reject(StatusCode::BAD_REQUEST, "invalid byte range"));
    }
    Ok(Some(value.into()))
}
async fn run_request(
    state: AppState,
    headers: HeaderMap,
    query: DownloadQuery,
    method: Method,
    preflight: bool,
    client_ip: IpAddr,
) -> Response {
    if state.ip_access_policy.read().unwrap().decide(client_ip) != AccessDecision::Allow {
        return reject(StatusCode::FORBIDDEN, "access denied by IP policy");
    }
    let (user, token) = match authorize(&headers, &state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    if !state.rate_limiter.check(
        &format!("download:{user}"),
        state.config().download.requests_per_minute,
        Instant::now(),
    ) {
        return reject(
            StatusCode::TOO_MANY_REQUESTS,
            "download rate limit exceeded",
        );
    }
    let url = match target(&query.url) {
        Ok(v) => v,
        Err(_) => return reject(
            StatusCode::BAD_REQUEST,
            "a public HTTPS download URL on port 443 without credentials or fragment is required",
        ),
    };
    let entry = url.host_str().unwrap().to_owned();
    let range = match range_header(&headers) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let config = state.config();
    let Some((permit, mut cancel)) = state.downloads.acquire(user, &config.download) else {
        return reject(
            StatusCode::TOO_MANY_REQUESTS,
            "download concurrency limit exceeded",
        );
    };
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(config.download.max_duration_secs);
    let result = tokio::select! {
        _=cancel.changed()=>return reject(StatusCode::FORBIDDEN,"download cancelled"),
        result=tokio::time::timeout_at(deadline,upstream(&state,&permit,url,method.clone(),range.as_deref()))=>result,
    };
    let (mut response, domain, hosts) = match result {
        Ok(Ok(v)) => v,
        other => {
            let r = match other {
                Ok(Err(r)) => *r,
                _ => reject(StatusCode::GATEWAY_TIMEOUT, "download deadline exceeded"),
            };
            let failed_host = state
                .downloads
                .active
                .lock()
                .unwrap()
                .get(&permit.id)
                .and_then(|a| a.hosts.last())
                .cloned()
                .unwrap_or_else(|| entry.clone());
            if state
                .database
                .download_event(
                    user,
                    &failed_host,
                    &entry,
                    true,
                    r.status() == StatusCode::FORBIDDEN,
                    &config,
                )
                .await
                .is_err()
            {
                return internal_error_response();
            }
            return r;
        }
    };
    let status = response.status();
    if !matches!(status.as_u16(), 200 | 206) {
        let _ = state
            .database
            .download_event(user, &domain, &entry, true, false, &config)
            .await;
        return reject(
            StatusCode::BAD_GATEWAY,
            "upstream did not return a downloadable response",
        );
    }
    if response
        .content_length()
        .is_some_and(|n| n > config.download.max_file_bytes)
    {
        let _ = state
            .database
            .download_event(user, &domain, &entry, true, true, &config)
            .await;
        return reject(
            StatusCode::PAYLOAD_TOO_LARGE,
            "download exceeds file size limit",
        );
    }
    if state
        .database
        .download_event(user, &domain, &entry, false, false, &config)
        .await
        .is_err()
    {
        return internal_error_response();
    }
    if preflight {
        return Json(json!({"allowed":true,"domain":domain,"entry_domain":entry,"redirect_domains":hosts,"content_length":response.content_length(),"max_file_bytes":config.download.max_file_bytes,"note":"Policy, DNS, token and quota are checked again when downloading. No file body was fetched."})).into_response();
    }
    let mut out = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"download\"",
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff");
    // Preserve only byte-oriented metadata. Never forward cookies, Location,
    // HTML content types, Content-Disposition or authentication challenges.
    for name in [
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::ETAG,
        header::LAST_MODIFIED,
        header::CONTENT_ENCODING,
    ] {
        if let Some(value) = response.headers().get(&name) {
            out = out.header(name, value);
        }
    }
    if method == Method::HEAD {
        return out.body(Body::empty()).unwrap();
    }
    let stream_failed = Arc::new(AtomicBool::new(false));
    let worker_failed = stream_failed.clone();
    let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(1);
    tokio::spawn(async move {
        let _permit = permit;
        let work = async {
            let mut total = 0u64;
            while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
                state
                    .database
                    .download_upstream(user, &domain, &entry, chunk.len() as u64)
                    .await
                    .map_err(|_| ())?;
                total = total.saturating_add(chunk.len() as u64);
                if total > config.download.max_file_bytes {
                    return Err(());
                }
                // Split before queueing/charging; memory and quota overshoot are bounded.
                for part in chunk.chunks(65536) {
                    let live = state.config();
                    if !live.download.enabled
                        || state
                            .database
                            .authenticate_download_token(&token)
                            .await
                            .map_err(|_| ())?
                            != Some(user)
                    {
                        return Err(());
                    }
                    for host in &hosts {
                        if state
                            .database
                            .download_blocked(host)
                            .await
                            .map_err(|_| ())?
                        {
                            return Err(());
                        }
                    }
                    state
                        .downloads
                        .throttle(part.len(), live.download.bandwidth_bytes_per_sec)
                        .await;
                    if state
                        .database
                        .authenticate_download_token(&token)
                        .await
                        .map_err(|_| ())?
                        != Some(user)
                    {
                        return Err(());
                    }
                    if !state
                        .database
                        .charge_download(user, &domain, &entry, part.len() as u64, &live)
                        .await
                        .map_err(|_| ())?
                    {
                        return Err(());
                    }
                    tx.send(Ok(bytes::Bytes::copy_from_slice(part)))
                        .await
                        .map_err(|_| ())?;
                }
            }
            Ok::<(), ()>(())
        };
        let failed = tokio::select! {
            result=tokio::time::timeout_at(deadline,work)=>!matches!(result,Ok(Ok(()))),
            _=cancel.changed()=>true,
            _=tx.closed()=>true,
        };
        if failed {
            let _ = state
                .database
                .download_stream_error(user, &domain, &entry)
                .await;
            worker_failed.store(true, Ordering::Release);
        }
    });
    let body = Body::from_stream(stream::unfold(
        (rx, false, stream_failed),
        |(mut rx, reported, failed)| async move {
            if reported {
                return None;
            }
            match rx.recv().await {
                Some(v) => Some((v, (rx, false, failed))),
                None if failed.load(Ordering::Acquire) => Some((
                    Err(std::io::Error::other(
                        "download interrupted by policy, quota, timeout or upstream failure",
                    )),
                    (rx, true, failed),
                )),
                None => None,
            }
        },
    ));
    out.body(body).unwrap()
}
async fn fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DownloadQuery>,
    request: Request,
) -> Response {
    let ip = resolve_client_ip(&request, &state.config());
    let method = request.method().clone();
    run_request(state, headers, query, method, false, ip).await
}
async fn check(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DownloadQuery>,
    request: Request,
) -> Response {
    let ip = resolve_client_ip(&request, &state.config());
    run_request(state, headers, query, Method::HEAD, true, ip).await
}

async fn account(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(u)) => u.user_id,
        Ok(None) => return unauthorized_response(),
        Err(_) => return internal_error_response(),
    };
    match async {Ok::<_,anyhow::Error>(json!({"enabled":state.config().download.enabled,"allowed":state.database.download_allowed(user).await?,"tokens":state.database.download_tokens(user).await?}))}.await {Ok(v)=>Json(v).into_response(),Err(_)=>internal_error_response()}
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenRequest {
    name: String,
    days: u32,
}
async fn create_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<TokenRequest>,
) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(u)) => u.user_id,
        Ok(None) => return unauthorized_response(),
        Err(_) => return internal_error_response(),
    };
    if r.name.trim().is_empty()
        || r.name.len() > 80
        || r.name.chars().any(char::is_control)
        || !(1..=365).contains(&r.days)
    {
        return reject(
            StatusCode::BAD_REQUEST,
            "invalid token name or lifetime (1–365 days)",
        );
    }
    match state
        .database
        .create_download_token(user, r.name.trim(), r.days)
        .await
    {
        Ok(Some(v)) => (
            StatusCode::CREATED,
            [(header::CACHE_CONTROL, "no-store")],
            Json(v),
        )
            .into_response(),
        Ok(None) => forbidden(),
        Err(_) => reject(
            StatusCode::CONFLICT,
            "cannot create token; at most 20 active tokens",
        ),
    }
}
async fn revoke_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let user = match authenticated_user(&headers, &state).await {
        Ok(Some(u)) => u.user_id,
        Ok(None) => return unauthorized_response(),
        Err(_) => return internal_error_response(),
    };
    match state.database.revoke_download_token(user, &id).await {
        Ok(()) => {
            state.downloads.cancel_user(user);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(_) => internal_error_response(),
    }
}
#[derive(Deserialize)]
struct StatsQuery {
    days: Option<u32>,
    user_id: Option<i64>,
    domain: Option<String>,
}
async fn admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Response {
    if let Err(r) = require_super_admin(&headers, &state).await {
        return r;
    }
    let domain = match q.domain {
        Some(d) if !d.is_empty() => match domain_input(&d) {
            Some(d) => Some(d),
            None => return reject(StatusCode::BAD_REQUEST, "invalid domain"),
        },
        _ => None,
    };
    match async {Ok::<_,anyhow::Error>(json!({"config":state.config().download,"permissions":state.database.download_permissions().await?,"blocks":state.database.download_blocks().await?,"usage":state.database.download_stats(q.days.unwrap_or(7).clamp(1,90),q.user_id,domain.as_deref()).await?,"active":state.downloads.list(),"cache_enabled":false,"row_limit":2000}))}.await {Ok(v)=>Json(v).into_response(),Err(_)=>internal_error_response()}
}
async fn update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(config): Json<DownloadConfig>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(e) = config.validate() {
        return reject(StatusCode::BAD_REQUEST, &e.to_string());
    }
    // Reuse the existing config validation and persistence path.
    let mut updated = (*state.config()).clone();
    updated.download = config;
    let response = update_admin_config(headers, State(state.clone()), Json(updated)).await;
    if response.status().is_success() {
        state.downloads.cancel_all();
    }
    let _ = identity;
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PermissionRequest {
    kind: String,
    subject_id: i64,
    policy: String,
}
async fn permission(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<PermissionRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if !matches!(r.kind.as_str(), "user" | "group")
        || !matches!(r.policy.as_str(), "allow" | "deny" | "inherit")
        || r.subject_id <= 0
    {
        return reject(StatusCode::BAD_REQUEST, "invalid permission");
    }
    match state
        .database
        .set_download_permission(&identity.username, &r.kind, r.subject_id, &r.policy)
        .await
    {
        Ok(true) => {
            state.downloads.cancel_all();
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => internal_error_response(),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockRequest {
    domain: String,
    subdomains: bool,
    reason: String,
    blocked: bool,
}
async fn block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<BlockRequest>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(domain) = domain_input(&r.domain) else {
        return reject(StatusCode::BAD_REQUEST, "invalid domain");
    };
    if r.reason.len() > 256 || r.reason.chars().any(char::is_control) {
        return reject(StatusCode::BAD_REQUEST, "invalid reason");
    }
    match state
        .database
        .set_download_block(
            &identity.username,
            &domain,
            r.subdomains,
            &r.reason,
            r.blocked,
        )
        .await
    {
        Ok(()) => {
            if r.blocked {
                state.downloads.cancel_domain(&domain, r.subdomains);
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(_) => internal_error_response(),
    }
}
async fn terminate(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let identity = match require_super_admin(&headers, &state).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if state
        .database
        .append_audit_log(&identity.username, "download_terminated", &id)
        .await
        .is_err()
    {
        return internal_error_response();
    }
    if let Some(a) = state.downloads.active.lock().unwrap().get(&id) {
        let _ = a.cancel.send(true);
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests;
