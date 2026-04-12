use crate::files;
use crate::forms;
use crate::models;
use crate::responses::AppError;
use crate::responses::HtmlResponse;
use crate::state::AppState;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

pub async fn index(State(mut state): State<AppState>) -> Response {
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

    HtmlResponse {
        title: "Robia Labs Ltd".to_string(),
        path: "src/templates/index.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
}

pub async fn submit_loan_application(
    State(mut state): State<AppState>,
    multipart: Multipart,
) -> Response {
    // Create contexts
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

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
            context.insert(
                "error_popup".to_string(),
                "A user with this phone number already exists.".to_string(),
            );
            return HtmlResponse {
                title: "Robia Labs Ltd".to_string(),
                path: "src/templates/index.html".to_string(),
                tera: &mut state.tera,
                context,
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

pub async fn submit_loan_application_redirect(State(_): State<AppState>) -> Response {
    // Redirect to index page if get request made to loan application endpoint
    axum::response::Redirect::to("/#quick-loan").into_response()
}

pub async fn login_page(State(mut state): State<AppState>) -> Response {
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Login".to_string());

    HtmlResponse {
        title: "Login".to_string(),
        path: "src/templates/login.html".to_string(),
        tera: &mut state.tera,
        context,
    }
    .into_response()
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
