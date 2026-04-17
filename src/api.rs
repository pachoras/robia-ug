use axum::{Json, extract::State, response::IntoResponse};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::verify_google_token, consts, mail, models, responses::AppError, session, state::AppState,
    workflows,
};

#[derive(Debug)]
pub struct ApiError(String);

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleLoginValues {
    pub token: String,
    pub application: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ApiResponseStatus {
    OK,
    ERROR,
    MISSING,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: ApiResponseStatus,
    pub data: Option<T>,
    pub token: Option<String>,
    pub error: Option<String>,
}

pub const AUTHENTICATION_COOKIE_NAME: &str = "auth_token";

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        let response = match self.status {
            ApiResponseStatus::OK => {
                Json(json!({"status": "OK", "data": self.data})).into_response()
            }
            ApiResponseStatus::ERROR => {
                Json(json!({"status": "ERROR", "error": self.error})).into_response()
            }
            ApiResponseStatus::MISSING => {
                Json(json!({"status": "MISSING", "error": self.error})).into_response()
            }
        };
        match self.token {
            Some(token) => {
                return session::set_http_cookie(
                    response,
                    AUTHENTICATION_COOKIE_NAME.to_string(),
                    token,
                );
            }
            None => return response,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub application_url: String,
}

pub async fn login_google(
    State(state): State<AppState>,
    Json(payload): Json<GoogleLoginValues>,
) -> ApiResponse<LoginSuccess> {
    // Decode JWT and get aud claim
    match verify_google_token(&payload.token).await {
        Ok(claims) => {
            // Check if user exists in database
            match models::User::find_by_email(&state.pool, &claims.email).await {
                Ok(mut user) => {
                    // Update the user's Google ID if it's not already set
                    if user.google_id.is_none() {
                        user.google_id = Some(claims.sub);
                        let mut tx = state
                            .pool
                            .begin()
                            .await
                            .map_err(|_| AppError {
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                message: None,
                            })
                            .unwrap();
                        if let Err(e) = models::User::update(&mut tx, user.id, &user).await {
                            mail::send_admin_error_email(&e.to_string())
                                .map_err(|_| AppError {
                                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                    message: None,
                                })
                                .unwrap();
                            tx.rollback()
                                .await
                                .map_err(|_| AppError {
                                    status_code: StatusCode::BAD_REQUEST,
                                    message: None,
                                })
                                .unwrap();
                            return ApiResponse {
                                status: ApiResponseStatus::ERROR,
                                data: None,
                                token: None,
                                error: Some(
                                    "Database error occurred. Please try again later.".to_string(),
                                ),
                            };
                        }
                    };
                    if payload.application != consts::APPLICATION_VARIANT_LOANS
                        && payload.application != consts::APPLICATION_VARIANT_PRO
                    {
                        log::warn!(
                            "Invalid application variant selected: {}",
                            payload.application
                        );
                        return ApiResponse {
                            status: ApiResponseStatus::ERROR,
                            data: None,
                            token: None,
                            error: Some("Invalid application selected.".to_string()),
                        };
                    } else if payload.application == consts::APPLICATION_VARIANT_PRO {
                        // Create auth token and return it as http-only cookie for pro user
                        match workflows::create_auth_token(
                            &state.pool,
                            user.id,
                            models::TokenTypeVariants::ProAuthentication,
                        )
                        .await
                        {
                            Ok(token) => {
                                log::info!(
                                    "User with email {} logged in successfully.",
                                    &claims.email
                                );
                                // Redirect to selected application page
                                let pro_application_url = std::env::var("PRO_APPLICATION_URL")
                                    .unwrap_or_else(|_| "http://localhost:4000".to_string());
                                // Set http token cookie and return
                                return ApiResponse {
                                    status: ApiResponseStatus::OK,
                                    data: Some(LoginSuccess {
                                        application_url: format!("{}/login", pro_application_url),
                                    }),
                                    error: None,
                                    token: Some(token.token),
                                };
                            }
                            Err(e) => {
                                log::error!(
                                    "Error creating auth token for user with email {}: {}",
                                    &claims.email,
                                    e
                                );
                                return ApiResponse {
                                    status: ApiResponseStatus::ERROR,
                                    data: None,
                                    token: None,
                                    error: Some("Could not log in at this time.".to_string()),
                                };
                            }
                        }
                    } else if payload.application == consts::APPLICATION_VARIANT_LOANS {
                        // Create auth token and return it as http-only cookie
                        match workflows::create_auth_token(
                            &state.pool,
                            user.id,
                            models::TokenTypeVariants::LoansAuthentication,
                        )
                        .await
                        {
                            Ok(token) => {
                                log::info!(
                                    "User with email {} logged in successfully.",
                                    &claims.email
                                );
                                // Redirect to selected application page
                                let loan_application_url = std::env::var("LOAN_APPLICATION_URL")
                                    .unwrap_or_else(|_| "http://localhost:3000".to_string());
                                let uri = format!("{}/login", loan_application_url);
                                // Set http token cookie and return
                                return ApiResponse {
                                    status: ApiResponseStatus::OK,
                                    data: Some(LoginSuccess {
                                        application_url: uri,
                                    }),
                                    error: None,
                                    token: Some(token.token),
                                };
                            }
                            Err(e) => {
                                log::error!(
                                    "Error creating auth token for user with email {}: {}",
                                    &claims.email,
                                    e
                                );
                                return ApiResponse {
                                    status: ApiResponseStatus::ERROR,
                                    data: None,
                                    token: None,
                                    error: Some("Could not log in at this time.".to_string()),
                                };
                            }
                        }
                    } else {
                        log::error!(
                            "Invalid application variant selected: {}",
                            payload.application
                        );
                        return ApiResponse {
                            status: ApiResponseStatus::ERROR,
                            data: None,
                            token: None,
                            error: Some("Invalid application selected.".to_string()),
                        };
                    }
                }
                Err(sqlx::Error::RowNotFound) => {
                    return ApiResponse {
                        status: ApiResponseStatus::MISSING,
                        data: None,
                        token: None,
                        error: Some(
                            "No user with this email found. Please sign up first.".to_string(),
                        ),
                    };
                }
                Err(e) => {
                    log::error!("Database error checking for user by email: {}", e);
                    mail::send_admin_error_email(&e.to_string()).unwrap_or_else(|_| ());
                    return ApiResponse {
                        status: ApiResponseStatus::ERROR,
                        data: None,
                        token: None,
                        error: Some("Database error occurred. Please try again later.".to_string()),
                    };
                }
            }
        }
        Err(err) => {
            log::error!("Error decoding JWT: {}", err);
            mail::send_admin_error_email(&err.to_string()).unwrap_or_else(|_| ());
            return ApiResponse {
                status: ApiResponseStatus::ERROR,
                data: None,
                token: None,
                error: Some("Invalid authentication token.".to_string()),
            };
        }
    }
}
