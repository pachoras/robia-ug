use crate::consts;
use crate::forms;
use crate::models;
use crate::responses::AppError;
use crate::responses::ErrorPopupResponse;
use crate::responses::HtmlResponse;
use crate::responses::SuccessPopupResponse;
use crate::session;
use crate::state::AppState;
use crate::workflows;
use axum::Form;
use axum::extract::Path;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde::Deserialize;
use serde::Serialize;

pub async fn index(State(mut state): State<AppState>) -> Response {
    let context = std::collections::HashMap::new();

    HtmlResponse {
        title: "Robia Labs Ltd".to_string(),
        path: "src/templates/index.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn register_loan_application(
    State(mut state): State<AppState>,
    multipart: Multipart,
) -> Response {
    // Validate form fields
    let (user_data, profile_data) = match forms::get_seeker_registration_form_data(multipart).await
    {
        Ok(data) => data,
        Err(e) => {
            log::error!("Form validation error: {}", e);
            return AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some(e.to_string()),
            }
            .into_response();
        }
    };

    match workflows::register_user(
        &state.pool,
        state.s3_client,
        &mut state.tera,
        &user_data,
        &profile_data,
    )
    .await
    {
        Ok(response) => response.into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn register_provider_application(
    State(mut state): State<AppState>,
    multipart: Multipart,
) -> Response {
    // Validate form fields
    let (user_data, profile_data) =
        match forms::get_provider_registration_form_data(multipart).await {
            Ok(data) => data,
            Err(e) => {
                log::error!("Form validation error: {}", e);
                return AppError {
                    status_code: StatusCode::BAD_REQUEST,
                    message: Some(
                        "Unable to save provider details at this time. Please try again later"
                            .to_string(),
                    ),
                }
                .into_response();
            }
        };

    match workflows::register_provider(
        &state.pool,
        state.s3_client,
        &mut state.tera,
        &user_data,
        &profile_data,
    )
    .await
    {
        Ok(response) => response.into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn login_page(State(mut state): State<AppState>) -> Response {
    let context = std::collections::HashMap::new();
    HtmlResponse {
        title: "Login".to_string(),
        path: "src/templates/login.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn handle_login(
    State(mut state): State<AppState>,
    Form(data): Form<forms::LoginData>,
) -> Response {
    // Check if user exists
    match models::User::find_by_email(&state.pool, &data.email).await {
        Ok(user) => {
            // Verify password
            if user.is_enabled
                && crate::utils::password_matches_hash(
                    &data.password,
                    &user.salt,
                    &user.password_hash,
                )
            {
                // Get selected application
                let app = data
                    .application
                    .unwrap_or_else(|| consts::APPLICATION_VARIANT_LOANS.to_string());
                if app != consts::APPLICATION_VARIANT_LOANS
                    && app != consts::APPLICATION_VARIANT_PRO
                {
                    log::warn!("Invalid application variant selected: {}", app);
                    return ErrorPopupResponse {
                        message: "Invalid application selected.".to_string(),
                        tera: &mut state.tera,
                        path: "src/templates/login.html",
                        context: std::collections::HashMap::new(),
                    }
                    .into_response();
                } else if app == consts::APPLICATION_VARIANT_PRO {
                    // Check if user has pro access
                    match models::ProviderProfile::find_by_user_id(&state.pool, user.id).await {
                        Ok(_) => {
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
                                        data.email
                                    );
                                    // Redirect to selected application page
                                    let pro_application_url = std::env::var("PRO_APPLICATION_URL")
                                        .unwrap_or_else(|_| "http://localhost:4000".to_string());
                                    let uri =
                                        format!("{}/login/{}", pro_application_url, token.token);
                                    // Set http token cookie and redirect
                                    return session::set_http_cookie(
                                        axum::response::Redirect::to(&uri).into_response(),
                                        "auth_token".to_string(),
                                        token.token.clone(),
                                    );
                                }
                                Err(e) => {
                                    log::error!(
                                        "Error creating auth token for user with email {}: {}",
                                        data.email,
                                        e
                                    );
                                    return AppError {
                                        status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                        message: Some("Could not log in at this time.".to_string()),
                                    }
                                    .into_response();
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Error fetching user profile for user ID {}: {}",
                                user.id,
                                e
                            );
                            return ErrorPopupResponse {
                                message: "Invalid email or password.".to_string(),
                                tera: &mut state.tera,
                                path: "src/templates/login.html",
                                context: std::collections::HashMap::new(),
                            }
                            .into_response();
                        }
                    }
                } else if app == consts::APPLICATION_VARIANT_LOANS {
                    // Create auth token and return it as http-only cookie
                    match workflows::create_auth_token(
                        &state.pool,
                        user.id,
                        models::TokenTypeVariants::LoansAuthentication,
                    )
                    .await
                    {
                        Ok(token) => {
                            log::info!("User with email {} logged in successfully.", data.email);
                            // Redirect to selected application page
                            let loan_application_url = std::env::var("LOAN_APPLICATION_URL")
                                .unwrap_or_else(|_| "http://localhost:3000".to_string());
                            let uri = format!("{}/login/{}", loan_application_url, token.token);
                            // Set http token cookie and redirect
                            return session::set_http_cookie(
                                axum::response::Redirect::to(&uri).into_response(),
                                "auth_token".to_string(),
                                token.token.clone(),
                            );
                        }
                        Err(e) => {
                            log::error!(
                                "Error creating auth token for user with email {}: {}",
                                data.email,
                                e
                            );
                            return AppError {
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                message: Some("Could not log in at this time.".to_string()),
                            }
                            .into_response();
                        }
                    }
                }
            } else if !user.is_enabled {
                log::warn!("Login attempt from disabled user: {}", data.email);
                return ErrorPopupResponse {
                    message: "Account deactivated. Please contact support for assistance."
                        .to_string(),
                    tera: &mut state.tera,
                    path: "src/templates/login.html",
                    context: std::collections::HashMap::new(),
                }
                .into_response();
            } else {
                log::warn!("Invalid password for email: {}", data.email);
                return ErrorPopupResponse {
                    message: "Invalid email or password.".to_string(),
                    tera: &mut state.tera,
                    path: "src/templates/login.html",
                    context: std::collections::HashMap::new(),
                }
                .into_response();
            }
        }
        Err(_) => {
            log::warn!("Login attempt with non-existent email: {}", data.email);
            return ErrorPopupResponse {
                message: "Invalid email or password.".to_string(),
                tera: &mut state.tera,
                path: "src/templates/login.html",
                context: std::collections::HashMap::new(),
            }
            .into_response();
        }
    }
    let context = std::collections::HashMap::new();
    HtmlResponse {
        title: "Login".to_string(),
        path: "src/templates/login.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn forgot_password_page(State(mut state): State<AppState>) -> Response {
    let context = std::collections::HashMap::new();
    HtmlResponse {
        title: "Forgot Password".to_string(),
        path: "src/templates/forgot_password.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn verify_token(
    State(mut state): State<AppState>,
    Path(token): Path<String>,
) -> Response {
    match models::ApplicationToken::find_by_token(&state.pool, &token).await {
        Ok(app_token) => {
            // Validate registration token
            match app_token.verify().await {
                Ok(verified_token) => {
                    // Render change password page
                    let mut context = std::collections::HashMap::new();
                    context.insert("token".to_string(), verified_token.token.clone());
                    return HtmlResponse {
                        title: "Update password".to_string(),
                        path: "src/templates/change_password.html".to_string(),
                        tera: &mut state.tera,
                        context,
                    }
                    .into_response();
                }
                Err(e) => {
                    log::error!("Invalid token: {}", e);
                    return AppError {
                        status_code: StatusCode::BAD_REQUEST,
                        message: Some("Token has expired.".to_string()),
                    }
                    .into_response();
                }
            }
        }
        Err(e) => {
            log::error!("Invalid token: {}", e);
            return AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some("Token is invalid.".to_string()),
            }
            .into_response();
        }
    }
}

pub async fn handle_forgot_password(
    State(mut state): State<AppState>,
    Form(data): Form<forms::ForgotPasswordData>,
) -> Response {
    match models::User::find_by_email(&state.pool, &data.email).await {
        Ok(user) => match workflows::create_password_reset_token(&state.pool, &user).await {
            Ok(_) => {
                log::info!("Created password reset token for user ID: {}", user.id);
            }
            Err(e) => {
                log::error!("Error creating password reset token: {}", e);
            }
        },
        Err(e) => {
            log::error!("Error finding user by email: {}", e);
        }
    };
    // Redirect to forgot password page with generic success message to prevent email enumeration
    return SuccessPopupResponse {
        message: "If an account with that email exists, a password reset link has been sent."
            .to_string(),
        tera: &mut state.tera,
        path: "src/templates/forgot_password.html",
        context: std::collections::HashMap::new(),
    }
    .into_response();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePasswordData {
    token: String,
    new_password: String,
}

pub async fn update_password(
    State(mut state): State<AppState>,
    Form(data): Form<UpdatePasswordData>,
) -> Response {
    match workflows::update_password(
        &state.pool,
        &mut state.tera,
        &data.token,
        &data.new_password,
    )
    .await
    {
        Ok(result) => result.into_response(),
        Err(e) => e.into_response(),
    }
}

/* Tests */
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use crate::files;
    use crate::renderer::init_renderer;
    use crate::state::AppState;
    use axum::body::to_bytes;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use tokio::sync::RwLock;

    async fn make_state() -> AppState {
        AppState {
            tera: init_renderer(),
            pool: sqlx::PgPool::connect("postgres://user:password@localhost/test_db")
                .await
                .unwrap_or_else(|_| panic!("Failed to connect to the database")),
            s3_client: files::initialize_s3_client().await,
            rate_limit_bucket: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[tokio::test]
    async fn index_returns_200_with_html_body() {
        let response = index(State(make_state().await)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Robia Labs Ltd"));
    }

    #[tokio::test]
    async fn index_response_has_html_content_type() {
        let response = index(State(make_state().await)).await.into_response();
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    }

    #[tokio::test]
    async fn app_error_unauthorized_returns_200_with_login_page() {
        let err = AppError {
            status_code: StatusCode::UNAUTHORIZED,
            message: None,
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Sign In"));
    }

    #[tokio::test]
    async fn app_error_500_returns_500_with_error_page() {
        let err = AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: Some("test error".to_string()),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Oops! Something went wrong."));
    }

    #[tokio::test]
    async fn app_error_response_has_html_content_type() {
        let err = AppError {
            status_code: StatusCode::UNAUTHORIZED,
            message: None,
        };
        let response = err.into_response();
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    }
}
