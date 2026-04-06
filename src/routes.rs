use crate::auth::verify_google_token;
use crate::forms;
use crate::models;
use crate::renderer::{init_renderer, render_template};
use crate::state::AppState;
use crate::utils;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::http::status;
use axum::response::{Html, IntoResponse, Json, Response};
use serde_json::Value;
use serde_json::json;

pub async fn index(State(mut state): State<AppState>) -> Result<Html<String>, AppError> {
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

    match render_template(&mut state.tera, "src/templates/index.html", &context) {
        Ok(content) => Ok(Html(content)),
        Err(e) => {
            log::error!("Error rendering template: {}", e);
            return Err(AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("Could not render response at this time.".to_string()),
            ));
        }
    }
}

pub async fn submit_loan_application(
    State(mut state): State<AppState>,
    multipart: Multipart,
) -> Result<Html<String>, AppError> {
    // Create contexts
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

    // Validate form fields
    let (user_data, mut profile_data) =
        match forms::get_seeker_registration_form_data(multipart).await {
            Ok(data) => data,
            Err(e) => {
                log::warn!("Form validation error: {}", e);
                context.insert("error_popup".to_string(), e.to_string());
                let res = render_template(&mut state.tera, "src/templates/index.html", &context);
                return Ok(Html(res.unwrap()));
            }
        };

    // Check if user with phone number or national ID already exists
    match models::UserProfile::find_by_phone_number(&state.pool, &profile_data.phone_number).await {
        Ok(existing_profile) => {
            log::warn!(
                "User with phone number {} already exists.\n Please log in instead.",
                existing_profile.phone_number
            );
            context.insert(
                "error_popup".to_string(),
                "A user with this phone number already exists.".to_string(),
            );
            let res = render_template(&mut state.tera, "src/templates/index.html", &context);
            return Ok(Html(res.unwrap()));
        }
        Err(sqlx::Error::RowNotFound) => {
            // No user with this phone number, continue
        }
        Err(e) => {
            log::error!(
                "Database error checking for existing user by phone number: {}",
                e
            );
            context.insert(
                "error_popup".to_string(),
                "Could not check for existing user at this time.".to_string(),
            );
            let res = render_template(&mut state.tera, "src/templates/index.html", &context);
            return Ok(Html(res.unwrap()));
        }
    }

    // Create user
    match models::User::create_with_email(&state.pool, &user_data.email).await {
        Ok(user) => {
            log::info!("Created user with ID: {}", user.id);
            // Create user profile
            profile_data.user_id = user.id;
            match models::UserProfile::create(&state.pool, &state.s3_client, &profile_data).await {
                Ok(profile) => {
                    log::info!("Created user profile for user ID: {}", profile.user_id);
                    context.insert(
                    "success_popup".to_string(),
                    "Your loan application has been received, please check your email for further instructions."
                        .to_string(),
                    );
                }
                Err(sqlx::Error::Database(e)) => {
                    if e.code() == Some("23505".into()) {
                        log::warn!("Duplicate entry error: {}", e);
                        context.insert(
                            "error_popup".to_string(),
                            "A user with this profile already exists.".to_string(),
                        );
                    }
                    log::error!("Database error creating user profile: {}", e);
                }
                Err(e) => {
                    log::error!("Error creating user profile: {}", e);
                    context.insert(
                        "error_popup".to_string(),
                        "Could not create user profile at this time.".to_string(),
                    );
                }
            }
        }
        Err(sqlx::Error::Database(e)) => {
            if e.code() == Some("23505".into()) {
                log::warn!("Duplicate entry error: {}", e);
                context.insert(
                    "error_popup".to_string(),
                    "A user with this email already exists.".to_string(),
                );
            }
        }
        Err(e) => {
            log::error!("Unexpected error creating user: {}", e);
            context.insert(
                "error_popup".to_string(),
                "Could not create user at this time.".to_string(),
            );
        }
    }

    // Render response
    match render_template(&mut state.tera, "src/templates/index.html", &context) {
        Ok(content) => Ok(Html(content)),
        Err(e) => {
            log::error!("Error rendering template: {}", e);
            return Err(AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("Could not render response at this time.".to_string()),
            ));
        }
    }
}

pub async fn login_page(State(mut state): State<AppState>) -> Result<Html<String>, AppError> {
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Login".to_string());

    match render_template(&mut state.tera, "src/templates/login.html", &context) {
        Ok(content) => Ok(Html(content)),
        Err(e) => {
            return Err(AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                Some(e.to_string()),
            ));
        }
    }
}

pub async fn login_google(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Decode JWT and get aud claim
    match payload.get("token").and_then(|v| v.as_str()) {
        Some(token) => {
            match verify_google_token(token).await {
                Ok(claims) => {
                    // Check if user with email exists in database
                    match models::User::find_by_email(&state.pool, &claims.email).await {
                        Ok(profile) => {
                            log::info!(
                                "User with email {} found, ID: {}",
                                claims.email,
                                profile.user_id
                            );
                            // If user exists, create auth token and return it in response
                            let app = payload
                                .get("app")
                                .and_then(|v| v.as_str())
                                .unwrap_or("loans");
                            let user_auth_token = models::UserAuthToken::new(
                                profile.user_id,
                                app.to_string(),
                                utils::generate_random_string(32),
                            );
                            match models::UserAuthToken::create(&state.pool, &user_auth_token).await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    log::error!("Error creating user auth token: {}", e);
                                    return Json(
                                        json!({"status": "ERROR", "error": "Could not create auth token at this time."}),
                                    );
                                }
                            };
                            // Also save user google id in database for future reference
                            let mut update_profile = profile.clone();
                            update_profile.google_id = Some(claims.sub.clone());
                            match models::UserProfile::update(
                                &state.pool,
                                update_profile.id,
                                &update_profile,
                            )
                            .await
                            {
                                Ok(_) => {
                                    return Json(
                                        json!({"status": "OK", "auth_token": &user_auth_token.token}),
                                    );
                                }
                                Err(e) => {
                                    log::error!(
                                        "Error updating Google ID for user with email {}: {}",
                                        claims.email,
                                        e
                                    );
                                    return Json(
                                        json!({"status": "ERROR", "error": "Could not update user profile with Google ID."}),
                                    );
                                }
                            }
                        }
                        Err(sqlx::Error::RowNotFound) => {
                            return Json(
                                json!({"status": "NOT_FOUND", "error": "No user with this email found. Please sign up first."}),
                            );
                        }
                        Err(e) => {
                            log::error!("Database error checking for user by email: {}", e);
                            return Json(
                                json!({"status": "ERROR", "error": "Could not check for user at this time."}),
                            );
                        }
                    }
                }
                Err(err) => {
                    log::error!("Error decoding JWT: {}", err);
                    return Json(json!({"status": "ERROR", "error": "Invalid token"}));
                }
            }
        }
        None => {
            log::error!("Missing token in request payload");
            return Json(json!({"status": "MISSING", "error": "Missing token"}));
        }
    }
}

#[derive(Debug)]
pub struct AppError(pub StatusCode, pub Option<String>);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut tera = init_renderer();

        let mut context = std::collections::HashMap::new();
        let mut response = (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();

        if self.0 == StatusCode::NOT_FOUND {
            context.insert("title".to_string(), "404 Not Found".to_string());
            let body = render_template(&mut tera, "src/templates/404.html", &context);
            response = (StatusCode::NOT_FOUND, body).into_response();
        } else if self.0 == StatusCode::INTERNAL_SERVER_ERROR {
            context.insert("title".to_string(), "500 Internal Server Error".to_string());
            log::error!(
                "Internal server error: {}",
                self.1.unwrap_or_else(|| "Unknown error".to_string())
            );
            let body = render_template(&mut tera, "src/templates/500.html", &context);
            response = (StatusCode::INTERNAL_SERVER_ERROR, body).into_response();
        } else if self.0 == StatusCode::UNAUTHORIZED {
            context.insert("title".to_string(), "Login".to_string());
            let body = render_template(&mut tera, "src/templates/login.html", &context);
            response = (StatusCode::OK, body).into_response();
        }

        response
            .headers_mut()
            .insert("Content-Type", "text/html".parse().unwrap());
        response
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
                .unwrap(),
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
        let err = AppError(StatusCode::UNAUTHORIZED, None);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Sign In"));
    }

    #[tokio::test]
    async fn app_error_500_returns_500_with_error_page() {
        let err = AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("test error".to_string()),
        );
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Oops! Something went wrong."));
    }

    #[tokio::test]
    async fn app_error_response_has_html_content_type() {
        let err = AppError(StatusCode::UNAUTHORIZED, None);
        let response = err.into_response();
        assert_eq!(response.headers().get("content-type").unwrap(), "text/html");
    }
}
