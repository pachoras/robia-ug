use axum::{
    extract::FromRequestParts,
    http::{
        Response, StatusCode,
        header::{AUTHORIZATION, HeaderValue},
    },
    response::Redirect,
};

use crate::renderer::{init_renderer, render_template};
use crate::{models, routes::AppError};

pub struct ExtractAuthenticationCode(pub HeaderValue);

impl<S> FromRequestParts<S> for ExtractAuthenticationCode
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, AppError> {
        if let Some(auth_header) = parts.headers.get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str.trim_start_matches("Bearer ").to_string();
                    let pool = models::connect_to_db().await.unwrap();
                    let database_token = models::UserAuthToken::find_by_token(&pool, &token).await;
                    if database_token.is_err() {
                        return Err(AppError(
                            StatusCode::UNAUTHORIZED,
                            Some("Invalid authentication token".to_string()),
                        ));
                    }
                    let code = ExtractAuthenticationCode(
                        HeaderValue::from_str(&database_token.unwrap().token).unwrap(),
                    );
                    return Ok(code);
                } else {
                    return Err(AppError(
                        StatusCode::UNAUTHORIZED,
                        Some("Authorization header must start with 'Bearer '".to_string()),
                    ));
                }
            }
        }
        return Err(AppError(
            StatusCode::UNAUTHORIZED,
            Some("Missing or invalid Authorization header".to_string()),
        ));
    }
}
