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
    let base = repository_for_target(&c.upstreams, target)?;
    let mut u = reqwest::Url::parse(base).map_err(|_| ProxyError::InvalidUrl)?;
    let b = u.path().trim_end_matches('/');
    u.set_path(&format!("{b}/{p}"));
    u.set_query(request.uri().query());
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
}

fn repository_for_target<'a>(
    upstreams: &'a crate::config::Upstreams,
    target: &str,
) -> Result<&'a str, ProxyError> {
    Ok(match target {
        "alpine" => &upstreams.alpine,
        "openwrt" => &upstreams.openwrt,
        "termux" => &upstreams.termux,
        "debian" => &upstreams.debian,
        "ubuntu" => &upstreams.ubuntu,
        "fedora" => &upstreams.fedora,
        "archlinux" => &upstreams.archlinux,
        "opensuse" => &upstreams.opensuse,
        "void" => &upstreams.void,
        "gentoo" => &upstreams.gentoo,
        "freebsd" => &upstreams.freebsd,
        target => upstreams
            .additional_os
            .get(target)
            .map(String::as_str)
            .ok_or(ProxyError::UnsupportedTarget)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_fixed_and_additional_targets() {
        let upstreams = crate::config::Upstreams::default();
        assert_eq!(
            repository_for_target(&upstreams, "debian").unwrap(),
            "https://deb.debian.org/debian"
        );
        assert_eq!(
            repository_for_target(&upstreams, "kali").unwrap(),
            "https://http.kali.org/kali"
        );
        assert_eq!(
            repository_for_target(&upstreams, "ros").unwrap(),
            "https://packages.ros.org/ros2/ubuntu"
        );
        assert_eq!(
            repository_for_target(&upstreams, "solus").unwrap(),
            "https://cdn.getsol.us/repo"
        );
        assert!(matches!(
            repository_for_target(&upstreams, "not-a-repository"),
            Err(ProxyError::UnsupportedTarget)
        ));
    }
}
