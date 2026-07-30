use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
    response::Response,
};
use include_dir::{include_dir, Dir};

use crate::config::SiteConfig;

static WEB_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web/dist");

pub fn serve(path: &str, site: &SiteConfig, canonical_base_url: &str) -> Response {
    let normalized = normalize_path(path);
    let asset_path = if normalized.is_empty() {
        "index.html"
    } else {
        normalized.as_str()
    };

    if let Some(file) = WEB_DIST.get_file(asset_path) {
        return if asset_path == "index.html" {
            index_response(file.contents(), path, site, canonical_base_url)
        } else {
            asset_response(asset_path, file.contents())
        };
    }

    if let Some(file) = WEB_DIST.get_file("index.html") {
        return index_response(file.contents(), path, site, canonical_base_url);
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))
        .expect("static response should be valid")
}

fn index_response(
    bytes: &'static [u8],
    request_path: &str,
    site: &SiteConfig,
    canonical_base_url: &str,
) -> Response {
    let mut html = String::from_utf8_lossy(bytes).into_owned();
    let title = escape_html(site.title.trim());
    let description = escape_html(site.description.trim());
    let keywords = escape_html(&site.keywords.join(", "));
    let icon = escape_html(site.icon_url.trim());
    let canonical = escape_html(canonical_base_url.trim_end_matches('/'));
    html = html.replace(
        "<title>MirrorProxy</title>",
        &format!("<title>{title}</title>"),
    );
    html = html.replace(
        "<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\" />",
        &format!(
            "<link rel=\"icon\" href=\"{icon}\" />\n    <link rel=\"apple-touch-icon\" href=\"{icon}\" />"
        ),
    );
    let mut metadata = String::new();
    if !description.is_empty() {
        metadata.push_str(&format!(
            "    <meta name=\"description\" content=\"{description}\" />\n    <meta property=\"og:description\" content=\"{description}\" />\n"
        ));
    }
    if !keywords.is_empty() {
        metadata.push_str(&format!(
            "    <meta name=\"keywords\" content=\"{keywords}\" />\n"
        ));
    }
    metadata.push_str(&format!(
        "    <meta property=\"og:title\" content=\"{title}\" />\n    <meta property=\"og:type\" content=\"website\" />\n"
    ));
    if !canonical.is_empty() {
        metadata.push_str(&format!(
            "    <link rel=\"canonical\" href=\"{canonical}\" />\n    <meta property=\"og:url\" content=\"{canonical}\" />\n"
        ));
    }
    if is_private_page(request_path) {
        metadata.push_str("    <meta name=\"robots\" content=\"noindex,nofollow\" />\n");
    }
    html = html.replace("  </head>", &format!("{metadata}  </head>"));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )
        .body(Body::from(html))
        .expect("index response should be valid")
}

fn is_private_page(path: &str) -> bool {
    path == "/admin"
        || path.starts_with("/admin/")
        || path == "/login"
        || path.starts_with("/login/")
        || path == "/account"
        || path.starts_with("/account/")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
    let cache_control = if path == "index.html" {
        HeaderValue::from_static("no-cache")
    } else {
        HeaderValue::from_static("public, max-age=31536000, immutable")
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, cache_control)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type.as_ref())
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        )
        .body(Body::from(bytes))
        .expect("static asset response should be valid")
}
