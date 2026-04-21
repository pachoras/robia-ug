use axum::{Json, extract::State, response::IntoResponse};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::{ProvidesValidAuthentication, ProvidesValidSubscription, verify_google_token},
    consts, mail,
    models::{self, ProviderProfile, TokenTypeVariants},
    state::AppState,
    workflows,
};

#[derive(Debug, Serialize, Deserialize)]
pub enum ApiErrorTypes {
    InvalidToken,
    DatabaseError,
    InvalidApplication,
    UserNotFound,
    AuthenticationFailed,
    SubscriptionInvalid,
    InternalServerError,
}

#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub error_type: ApiErrorTypes,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error_type {
            ApiErrorTypes::InvalidToken => "INVALID_TOKEN",
            ApiErrorTypes::DatabaseError => "DATABASE_ERROR",
            ApiErrorTypes::InvalidApplication => "INVALID_APPLICATION",
            ApiErrorTypes::UserNotFound => "USER_NOT_FOUND",
            ApiErrorTypes::AuthenticationFailed => "AUTHENTICATION_FAILED",
            ApiErrorTypes::SubscriptionInvalid => "SUBSCRIPTION_INVALID",
            ApiErrorTypes::InternalServerError => "INTERNAL_SERVER_ERROR",
        };
        Json(json!({"status": status, "error": self.message})).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ApiResponseStatus {
    OK,
    MISSING,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: ApiResponseStatus,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        match self.status {
            ApiResponseStatus::OK => {
                Json(json!({"status": "OK", "data": self.data})).into_response()
            }
            ApiResponseStatus::MISSING => {
                Json(json!({"status": "MISSING", "error": self.error})).into_response()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse<T> {
    pub data: Option<T>,
}

impl<T> IntoResponse for SuccessResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        Json(json!({"status": "OK", "data": self.data})).into_response()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GoogleLoginValues {
    pub token: String,
    pub application: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub application_url: String,
    pub token: String,
}

pub async fn login_google(
    State(state): State<AppState>,
    Json(payload): Json<GoogleLoginValues>,
) -> Result<SuccessResponse<LoginSuccess>, ApiError> {
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
                            .map_err(|_| ApiError {
                                message: "Database error".to_string(),
                                error_type: ApiErrorTypes::DatabaseError,
                            })
                            .unwrap();
                        if let Err(e) = models::User::update(&mut tx, user.id, &user).await {
                            mail::send_admin_error_email(&e.to_string())
                                .map_err(|_| ApiError {
                                    message: "Database error".to_string(),
                                    error_type: ApiErrorTypes::DatabaseError,
                                })
                                .unwrap();
                            tx.rollback()
                                .await
                                .map_err(|_| ApiError {
                                    message: "Database error".to_string(),
                                    error_type: ApiErrorTypes::DatabaseError,
                                })
                                .unwrap();
                            return Err(ApiError {
                                message: "A Database error".to_string(),
                                error_type: ApiErrorTypes::DatabaseError,
                            });
                        }
                    };
                    // Match the selected login portal
                    if payload.application != consts::APPLICATION_VARIANT_LOANS
                        && payload.application != consts::APPLICATION_VARIANT_PRO
                    {
                        log::warn!(
                            "Invalid application variant selected: {}",
                            payload.application
                        );
                        return Err(ApiError {
                            message: "Invalid application selected.".to_string(),
                            error_type: ApiErrorTypes::DatabaseError,
                        });
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
                                // Redirect to selected application page
                                let pro_application_url = std::env::var("PRO_APPLICATION_URL")
                                    .unwrap_or_else(|_| "http://localhost:4000".to_string());
                                // Set http token cookie and return
                                return Ok(SuccessResponse {
                                    data: Some(LoginSuccess {
                                        application_url: format!("{}/login", pro_application_url),
                                        token: token.token,
                                    }),
                                });
                            }
                            Err(e) => {
                                log::error!(
                                    "Error creating auth token for user with email {}: {}",
                                    &claims.email,
                                    e
                                );
                                return Err(ApiError {
                                    message: "Could not log in at this time.".to_string(),
                                    error_type: ApiErrorTypes::AuthenticationFailed,
                                });
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
                                return Ok(SuccessResponse {
                                    data: Some(LoginSuccess {
                                        application_url: uri,
                                        token: token.token,
                                    }),
                                });
                            }
                            Err(e) => {
                                log::error!(
                                    "Error creating auth token for user with email {}: {}",
                                    &claims.email,
                                    e
                                );
                                return Err(ApiError {
                                    message: "Could not log in at this time.".to_string(),
                                    error_type: ApiErrorTypes::AuthenticationFailed,
                                });
                            }
                        }
                    } else {
                        log::error!(
                            "Invalid application variant selected: {}",
                            payload.application
                        );
                        return Err(ApiError {
                            message: "Invalid application selected.".to_string(),
                            error_type: ApiErrorTypes::InvalidApplication,
                        });
                    }
                }
                Err(sqlx::Error::RowNotFound) => {
                    return Err(ApiError {
                        message: "No user with this email found. Please sign up first.".to_string(),
                        error_type: ApiErrorTypes::UserNotFound,
                    });
                }
                Err(e) => {
                    log::error!("Database error checking for user by email: {}", e);
                    mail::send_admin_error_email(&e.to_string()).unwrap_or_else(|_| ());
                    return Err(ApiError {
                        message: "Database error occurred. Please try again later.".to_string(),
                        error_type: ApiErrorTypes::DatabaseError,
                    });
                }
            }
        }
        Err(err) => {
            log::error!("Error decoding JWT: {}", err);
            mail::send_admin_error_email(&err.to_string()).unwrap_or_else(|_| ());
            return Err(ApiError {
                message: "Invalid authentication token.".to_string(),
                error_type: ApiErrorTypes::AuthenticationFailed,
            });
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticationSuccess {
    pub token: String,
}
/// Endpoint for validating tokens.
pub async fn authenticate_application(
    ProvidesValidAuthentication(auth_token): ProvidesValidAuthentication,
) -> Result<SuccessResponse<AuthenticationSuccess>, ApiError> {
    // Check token type
    if !(auth_token.token_type == TokenTypeVariants::LoansAuthentication as i32
        || auth_token.token_type == TokenTypeVariants::ProAuthentication as i32)
    {
        return Err(ApiError {
            message: "Invalid token type.".to_string(),
            error_type: ApiErrorTypes::AuthenticationFailed,
        });
    }
    return Ok(SuccessResponse {
        data: Some(AuthenticationSuccess {
            token: auth_token.token,
        }),
    });
}
/// Endpoint for getting a valid profile.
pub async fn get_valid_provider_profile(
    ProvidesValidSubscription(provider_profile): ProvidesValidSubscription,
) -> Result<SuccessResponse<ProviderProfile>, ApiError> {
    return Ok(SuccessResponse {
        data: Some(provider_profile),
    });
}
