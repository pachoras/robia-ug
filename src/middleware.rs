use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Request, State},
    http::{self},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::{self, RateLimits, StateError, read_limit_values, write_limit_values};

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

const IP_RATE_LIMIT: u64 = 10;
const RATE_LIMIT_TIMEOUT: u64 = 60;

/// Update the ip rate limit, or return an error if it's been exceeded
pub async fn check_or_update_ip_rate_limit(
    limits: RateLimits,
    ip_address: String,
) -> Result<(), StateError> {
    let (current_limit, current_timeout) = read_limit_values(&limits, &ip_address).await;
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StateError(e.to_string()))
        .unwrap()
        .as_secs()
        - current_timeout;
    if current_limit > IP_RATE_LIMIT {
        if duration > RATE_LIMIT_TIMEOUT {
            // If enough time has elapsed, reset the counter
            write_limit_values(limits, ip_address, 1).await;
        }
        return Err(StateError("User has been rate limited".to_string()));
    } else {
        // Update the limit
        write_limit_values(limits, ip_address, current_limit + 1).await;
    }
    Ok(())
}
/// Middleware fn for setting default rate limit values
pub async fn default_rate_limit(
    State(state): State<state::AppState>,
    request: Request,
    next: Next,
) -> Response {
    match request.headers().get("X-REAL-IP") {
        Some(real_ip) => {
            // Update the state bucket
            match check_or_update_ip_rate_limit(
                state.rate_limit_bucket,
                real_ip.to_str().unwrap().to_string(),
            )
            .await
            {
                Ok(_) => return next.run(request).await,
                Err(e) => return e.into_response(),
            }
        }
        None => {
            // Continue with request if header is not present
            return next.run(request).await;
        }
    }
}
