use axum::body::Body;
use axum::http::HeaderValue;
use axum::http::header;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize)]
pub struct HttpOnlyCookie {
    pub name: String,
    pub value: String,
}

impl HttpOnlyCookie {
    fn into_string(self) -> String {
        let mut cookie_string = format!("{}={}", self.name, self.value);
        cookie_string.push_str("; SameSite=Strict");
        cookie_string.push_str("; Max-Age=604800"); // 7 days in seconds
        cookie_string.push_str("; Path=/");
        cookie_string.push_str("; Secure");
        cookie_string.push_str("; HttpOnly");
        cookie_string
    }
    fn from_string(cookie_str: &str) -> Option<HttpOnlyCookie> {
        let parts: Vec<&str> = cookie_str.split(';').next()?.split('=').collect();
        if parts.len() == 2 {
            Some(HttpOnlyCookie {
                name: parts[0].trim().to_string(),
                value: parts[1].trim().to_string(),
            })
        } else {
            None
        }
    }
}

/// Reads the "user_session_id" cookie from the request headers.
/// Returns a list containing the key and value pairs of the cookie.
pub fn read_http_cookie(req: Request<Body>) -> Result<HttpOnlyCookie, StatusCode> {
    match req.headers().get(header::COOKIE) {
        Some(cookie_header) => match cookie_header.to_str() {
            Ok(cookies) => match HttpOnlyCookie::from_string(cookies) {
                Some(http_cookie) => Ok(http_cookie),
                None => {
                    log::error!("No valid cookies found in header");
                    Err(StatusCode::BAD_REQUEST)
                }
            },
            Err(e) => {
                let msg = format!("Error reading cookie header: {}", e.to_string());
                log::error!("{}", msg);
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        None => {
            log::error!("No cookie header found");
            return Err(StatusCode::BAD_REQUEST);
        }
    }
}

/// Sets a cookie in the response header. This is a simplified example and does not include attributes like
/// HttpOnly, Secure, etc.
pub fn set_http_cookie(mut response: Response, key: String, value: String) -> Response {
    let cookie = HttpOnlyCookie { name: key, value }.into_string();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    response
}
