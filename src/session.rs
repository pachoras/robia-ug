use axum::body::Body;
use axum::http::HeaderValue;
use axum::http::header;
use axum::http::{Request, StatusCode};
use axum::response::Response;

/// Reads the "user_session_id" cookie from the request headers.
/// Returns a list containing the key and value pairs of the cookie.
async fn read_cookie(req: Request<Body>) -> Result<Vec<(String, String)>, StatusCode> {
    if let Some(cookie_header) = req.headers().get(header::COOKIE) {
        if let Ok(cookies) = cookie_header.to_str() {
            let mut kv_pairs: Vec<(String, String)> = Vec::new();
            for cookie in cookies.split(';') {
                let cookie_parts: Vec<&str> = cookie.split('=').collect();
                if cookie_parts.len() == 2 {
                    kv_pairs.push((
                        cookie_parts[0].trim().to_string(),
                        cookie_parts[1].trim().to_string(),
                    ));
                    return Ok(kv_pairs);
                }
            }
        }
    }
    Err(StatusCode::BAD_REQUEST)
}

/// Sets a cookie in the response header. This is a simplified example and does not include attributes like
/// HttpOnly, Secure, etc.
async fn set_cookie(mut response: Response, cookie_value: String) -> Response {
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie_value).unwrap(),
    );
    response
}
