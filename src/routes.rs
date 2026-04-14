use crate::consts;
use crate::forms;
use crate::models;
use crate::responses::AppError;
use crate::responses::ErrorPopupResponse;
use crate::responses::HtmlResponse;
use crate::responses::SuccessPopupResponse;
use crate::session;
use crate::state::AppState;
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

pub async fn register_loan(State(mut state): State<AppState>, multipart: Multipart) -> Response {
    // Create contexts
    let context = std::collections::HashMap::new();

    // Validate form fields
    let (user_data, mut profile_data) =
        match forms::get_seeker_registration_form_data(multipart).await {
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

    // Check if user with phone number or national ID already exists
    match models::UserProfile::find_by_phone_number(&state.pool, &profile_data.phone_number).await {
        Ok(existing_profile) => {
            log::warn!(
                "User with phone number {} already exists.\n Please log in instead.",
                existing_profile.phone_number
            );
            return AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some("A user with this profile already exists.".to_string()),
            }
            .into_response();
        }
        Err(sqlx::Error::RowNotFound) => {
            // No user with this phone number, continue
        }
        Err(e) => {
            return AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: Some(format!(
                    "Could not check for existing user at this time: {}",
                    e
                )),
            }
            .into_response();
        }
    }

    // Create user
    match models::User::create_loan_applicant(&state.pool, &user_data.email).await {
        Ok(user) => {
            log::info!("Created user with ID: {}", user.id);
            // Create user profile
            profile_data.user_id = user.id;
            match models::UserProfile::create(&state.pool, &state.s3_client, &profile_data).await {
                Ok(profile) => {
                    log::info!("Created user profile for user ID: {}", profile.user_id);
                    return SuccessPopupResponse {
                        message: "Your loan application has been received, please check your email for further instructions.",
                        tera: &mut state.tera,
                        path: "src/templates/index.html",
                        context,
                    }.into_response();
                }
                Err(sqlx::Error::Database(e)) => {
                    if e.code() == Some("23505".into()) {
                        return AppError {
                            status_code: StatusCode::BAD_REQUEST,
                            message: Some("A user with this profile already exists.".to_string()),
                        }
                        .into_response();
                    }
                }
                Err(e) => {
                    log::error!("Error creating user profile: {}", e);
                    return AppError {
                        status_code: StatusCode::INTERNAL_SERVER_ERROR,
                        message: Some("Could not create user profile at this time.".to_string()),
                    }
                    .into_response();
                }
            }
        }
        Err(sqlx::Error::Database(e)) => {
            if e.code() == Some("23505".into()) {
                return AppError {
                    status_code: StatusCode::BAD_REQUEST,
                    message: Some("A user with this email already exists.".to_string()),
                }
                .into_response();
            }
        }
        Err(e) => {
            log::error!("Error creating user: {}", e);
            return AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: Some("Could not create user at this time.".to_string()),
            }
            .into_response();
        }
    }

    // Render response
    HtmlResponse {
        title: "Robia Labs Ltd".to_string(),
        path: "src/templates/index.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn register_loan_redirect(State(_): State<AppState>) -> Response {
    // Redirect to index page if get request made to loan application endpoint
    axum::response::Redirect::to("/#quick-loan").into_response()
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
            if crate::utils::password_matches_hash(&data.password, &user.salt, &user.password_hash)
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
                        message: "Invalid application selected.",
                        tera: &mut state.tera,
                        path: "src/templates/login.html",
                        context: std::collections::HashMap::new(),
                    }
                    .into_response();
                } else if app == consts::APPLICATION_VARIANT_PRO {
                    // Create auth token and return it as http-only cookie for pro user
                    match models::ApplicationToken::create_auth_token(
                        &state.pool,
                        user.id,
                        models::TokenTypeVariants::ProAuthentication,
                    )
                    .await
                    {
                        Ok(token) => {
                            log::info!("User with email {} logged in successfully.", data.email);
                            // Redirect to selected application page
                            let pro_application_url = std::env::var("PRO_APPLICATION_URL")
                                .unwrap_or_else(|_| "http://localhost:4000".to_string());
                            let uri = format!("{}/login/{}", pro_application_url, token.token);
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
                } else if app == consts::APPLICATION_VARIANT_LOANS {
                    // Create auth token and return it as http-only cookie
                    match models::ApplicationToken::create_auth_token(
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
            } else {
                log::warn!("Invalid password for email: {}", data.email);
                return ErrorPopupResponse {
                    message: "Invalid email or password.",
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
                message: "Invalid email or password.",
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
            // Validate app_token
            match app_token.verify(&state.pool).await {
                Ok(verified_token) => {
                    // Render change password page
                    let mut context = std::collections::HashMap::new();
                    context.insert("token".to_string(), verified_token.token);
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
        Ok(user) => {
            match models::ApplicationToken::create_password_reset_token(&state.pool, &user).await {
                Ok(_) => {
                    log::info!("Created password reset token for user ID: {}", user.id);
                    return SuccessPopupResponse {
                            message: "If an account with that email exists, a password reset link has been sent.",
                            tera: &mut state.tera,
                            path: "src/templates/forgot_password.html",
                            context: std::collections::HashMap::new(),
                    }
                    .into_response();
                }
                Err(e) => {
                    log::error!("Error creating password reset token: {}", e);
                    axum::response::Redirect::to("/#login").into_response()
                }
            }
        }
        Err(e) => {
            log::error!("Error finding user by email: {}", e);
            // Redirect to forgot password page with generic success message to prevent email enumeration
            axum::response::Redirect::to("/#login").into_response()
        }
    }
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
    match models::ApplicationToken::find_by_token(&state.pool, &data.token).await {
        Ok(token) => {
            // Validate token
            match token.verify(&state.pool).await {
                Ok(app_token) => {
                    // Validate new password
                    match forms::validate_password(&data.new_password).await {
                        Ok(_) => {
                            // Mark token as used
                            let _ = models::ApplicationToken::set_used(&state.pool, &app_token.id)
                                .await;
                            // Create new password hash
                            match models::User::find(&state.pool, app_token.user_id).await {
                                Ok(mut user) => {
                                    user.password_hash = crate::utils::get_password_hash(
                                        &data.new_password,
                                        &user.salt,
                                    );
                                    match models::User::update(&state.pool, user.id, &user).await {
                                        Ok(_) => {
                                            log::info!("Password updated for user ID: {}", user.id);
                                            // Redirect to login page with success message
                                            SuccessPopupResponse {
                                                message: "Your password has been updated successfully. Please log in.",
                                                tera: &mut state.tera,
                                                path: "src/templates/login.html",
                                                context: std::collections::HashMap::new(),
                                            }
                                            .into_response()
                                        }
                                        Err(e) => {
                                            log::error!("Error updating password: {}", e);
                                            return AppError {
                                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                                message: Some(
                                                    "Could not update password at this time."
                                                        .to_string(),
                                                ),
                                            }
                                            .into_response();
                                        }
                                    }
                                }
                                Err(_) => {
                                    return AppError {
                                        status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                        message: Some("Could not update password.".to_string()),
                                    }
                                    .into_response();
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Password validation error: {}", e);
                            let message: &'static str = Box::leak(e.to_string().into_boxed_str());
                            return ErrorPopupResponse {
                                message,
                                tera: &mut state.tera,
                                path: "src/templates/change_password.html",
                                context: {
                                    let mut c = std::collections::HashMap::new();
                                    c.insert("token".to_string(), app_token.token.clone());
                                    c
                                },
                            }
                            .into_response();
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error verifying registration token: {}", e);
                    return AppError {
                        status_code: StatusCode::BAD_REQUEST,
                        message: Some("Could not verify registration at this time.".to_string()),
                    }
                    .into_response();
                }
            }
        }
        Err(e) => {
            log::error!("Invalid registration token: {}", e);
            return AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some("Token has expired.".to_string()),
            }
            .into_response();
        }
    }
}

/* Tests */
#[cfg(test)]
mod tests {
    use super::*;
    use crate::files;
    use crate::renderer::init_renderer;
    use crate::state::AppState;
    use axum::body::to_bytes;
    use axum::extract::State;
    use axum::response::IntoResponse;

    async fn make_state() -> AppState {
        AppState {
            tera: init_renderer(),
            pool: sqlx::PgPool::connect("postgres://user:password@localhost/test_db")
                .await
                .unwrap_or_else(|_| panic!("Failed to connect to the database")),
            s3_client: files::initialize_s3_client().await,
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
