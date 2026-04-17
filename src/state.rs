use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::response::{Html, IntoResponse, Response};
use tokio::sync::RwLock;

pub type RateLimits = Arc<RwLock<HashMap<String, Vec<String>>>>;

#[derive(Debug)]
pub struct StateError(pub String);

impl IntoResponse for StateError {
    fn into_response(self) -> Response {
        Html(self.0).into_response()
    }
}

// Application state
#[derive(Clone)]
pub struct AppState {
    pub tera: tera::Tera,
    pub pool: sqlx::PgPool,
    pub s3_client: aws_sdk_s3::Client,
    pub rate_limit_bucket: RateLimits,
}

/// Update rate limits from global store.
/// Timeout is automatically updated on each insert
pub async fn write_limit_values(limits: RateLimits, ip_address: String, rate_limit: u64) {
    // Create new list and map values
    let mut new_limit = Vec::new();
    new_limit.insert(0, rate_limit.to_string());
    new_limit.insert(
        1,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| StateError(e.to_string()))
            .unwrap()
            .as_secs()
            .to_string(),
    );
    let mut write_lock = limits.write().await;
    write_lock.insert(ip_address, new_limit);
}

/// Read values from mutable limits
pub async fn read_limit_values(limits: &RateLimits, ip_address: &String) -> (u64, u64) {
    let read_lock = limits.read().await;
    let data = read_lock.get(ip_address);
    if data.is_some() {
        // Unwrap existing values
        let rate_limit = data.unwrap().get(0).unwrap();
        let timeout = data.unwrap().get(1).unwrap();
        return (rate_limit.parse().unwrap(), timeout.parse().unwrap());
    }
    (0, 0)
}
