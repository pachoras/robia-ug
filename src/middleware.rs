use axum::{extract::Request, http, middleware::Next, response::Response};

pub async fn default_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        http::header::X_FRAME_OPTIONS,
        http::HeaderValue::from_static("DENY"),
    );
    response.headers_mut().insert(
        http::header::X_XSS_PROTECTION,
        http::HeaderValue::from_static("1; mode=block"),
    );
    response.headers_mut().insert(
        http::header::REFERRER_POLICY,
        http::HeaderValue::from_static("no-referrer"),
    );
    response
}

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "Cross-Origin-Opener-Policy",
        http::HeaderValue::from_static("same-origin-allow-popups"),
    );
    response
}

pub async fn cache_control_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        http::header::CACHE_CONTROL,
        http::HeaderValue::from_static("public, max-age=300, s-maxage=180, must-revalidate"),
    );
    response
}
