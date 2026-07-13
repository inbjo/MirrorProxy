use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("os") {
        return Err(ProxyError::Disabled("os"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy OS static repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let c = state.config();
    if !c.is_enabled("os") {
        return Err(ProxyError::Disabled("os"));
    }
    let (target, p) = path
        .trim_start_matches('/')
        .split_once('/')
        .ok_or(ProxyError::InvalidUrl)?;
    if p.is_empty()
        || p.contains('\\')
        || p.split('/')
            .any(|v| v.is_empty() || v == "." || v == ".." || v.contains('\0'))
    {
        return Err(ProxyError::InvalidUrl);
    }
    let base = match target {
        "alpine" => &c.upstreams.alpine,
        "openwrt" => &c.upstreams.openwrt,
        "termux" => &c.upstreams.termux,
        "debian" => &c.upstreams.debian,
        "ubuntu" => &c.upstreams.ubuntu,
        "fedora" => &c.upstreams.fedora,
        "archlinux" => &c.upstreams.archlinux,
        "opensuse" => &c.upstreams.opensuse,
        "void" => &c.upstreams.void,
        "gentoo" => &c.upstreams.gentoo,
        "freebsd" => &c.upstreams.freebsd,
        target => c
            .upstreams
            .additional_os
            .get(target)
            .ok_or(ProxyError::UnsupportedTarget)?,
    };
    let mut u = reqwest::Url::parse(base).map_err(|_| ProxyError::InvalidUrl)?;
    let b = u.path().trim_end_matches('/');
    u.set_path(&format!("{b}/{p}"));
    u.set_query(request.uri().query());
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
}
#[cfg(test)]
mod tests {
    #[test]
    fn documents_fixed_targets() {
        assert!(matches!(
            "alpine",
            "alpine"
                | "openwrt"
                | "termux"
                | "debian"
                | "ubuntu"
                | "fedora"
                | "archlinux"
                | "opensuse"
                | "void"
                | "gentoo"
                | "freebsd"
                | "kali"
                | "rocky"
                | "alma"
                | "manjaro"
                | "msys2"
        ));
    }
}
