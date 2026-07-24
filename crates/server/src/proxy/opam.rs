use super::ProxyError;
use crate::{proxy, AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
pub async fn root(State(state): State<AppState>) -> Result<Response, ProxyError> {
    if !state.config().is_enabled("opam") {
        return Err(ProxyError::Disabled("opam"));
    }
    Response::builder()
        .status(200)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from("MirrorProxy opam repository proxy\n"))
        .map_err(|_| ProxyError::InvalidHeader)
}
pub async fn proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ProxyError> {
    let c = state.config();
    if !c.is_enabled("opam") {
        return Err(ProxyError::Disabled("opam"));
    }
    let u = repository_url(&c.upstreams.opam, &path, request.uri().query())?;
    proxy::forward(&state, request.method().clone(), u, request.headers()).await
}

fn repository_url(base: &str, path: &str, query: Option<&str>) -> Result<reqwest::Url, ProxyError> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || path.contains('\\')
        || path.split('/').any(|segment| {
            segment.is_empty() || matches!(segment, "." | "..") || segment.contains('\0')
        })
    {
        return Err(ProxyError::InvalidUrl);
    }
    let mut url =
        reqwest::Url::parse(proxy::select_upstream(base)?).map_err(|_| ProxyError::InvalidUrl)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}/{path}"));
    url.set_query(query);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_opam_repository_paths() {
        let url = repository_url(
            "https://opam.example/repository",
            "packages/dune/dune.3.17.0/opam",
            None,
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://opam.example/repository/packages/dune/dune.3.17.0/opam"
        );
    }

    #[test]
    fn rejects_unsafe_opam_paths() {
        for path in ["", "../packages", "packages//dune", "packages\\dune"] {
            assert!(repository_url("https://opam.example", path, None).is_err());
        }
    }
}
