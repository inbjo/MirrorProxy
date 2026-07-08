use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use include_dir::{include_dir, Dir};

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web/dist");

pub fn serve(path: &str) -> Response {
    let normalized = normalize_path(path);
    let asset_path = if normalized.is_empty() {
        "index.html"
    } else {
        normalized.as_str()
    };

    if let Some(file) = WEB_DIST.get_file(asset_path) {
        return asset_response(asset_path, file.contents());
    }

    if let Some(file) = WEB_DIST.get_file("index.html") {
        return asset_response("index.html", file.contents());
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))
        .expect("static response should be valid")
}

fn normalize_path(path: &str) -> String {
    path.trim_start_matches('/')
        .split('/')
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
}

fn asset_response(path: &str, bytes: &'static [u8]) -> Response {
    let content_type = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type.as_ref())
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        )
        .body(Body::from(bytes))
        .expect("static asset response should be valid")
}
