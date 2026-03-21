use crate::renderer::{init_renderer, render_template};
use crate::state::{self, AppState};
use crate::utils;
use crate::{auth, models};
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

/// Helper function to create a header map with the correct content type for HTML responses.
fn get_headers() -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/html".parse()?);
    Ok(headers)
}

pub async fn index(State(mut state): State<AppState>) -> impl IntoResponse {
    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

    let result = render_template(&mut state.tera, "src/templates/index.html", &context);
    if result.is_err() {
        return Err(AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(result.err().unwrap()),
        ));
    }
    let content = result.unwrap();

    Ok((get_headers().unwrap_or_else(|_| HeaderMap::new()), content))
}

/// Helper function to extract and validate registration form data from the multipart request.
async fn get_registration_data(
    mut multipart: Multipart,
) -> Result<(models::UserData, models::UserProfileData), std::collections::HashMap<String, String>>
{
    let mut user_data = models::UserData::new();
    let mut profile_data = models::UserProfileData::new();
    let mut context = std::collections::HashMap::new();
    while let Some(field) = multipart.next_field().await.unwrap() {
        // Get form fields
        let name = field.name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        if name == "email" {
            user_data.email = String::from_utf8(data.to_vec()).unwrap();
            if user_data.email.is_empty() {
                context.insert("email_error".to_string(), "Email is required".to_string());
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "full_name" {
            profile_data.full_name = String::from_utf8(data.to_vec()).unwrap();
            if profile_data.full_name.is_empty() {
                context.insert(
                    "full_name_error".to_string(),
                    "Full name is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "national_id" {
            profile_data.national_id = String::from_utf8(data.to_vec()).unwrap();
            if profile_data.national_id.is_empty() {
                context.insert(
                    "national_id_error".to_string(),
                    "National ID is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "phone_number" {
            profile_data.phone_number = String::from_utf8(data.to_vec()).unwrap();
            if profile_data.phone_number.is_empty() {
                context.insert(
                    "phone_number_error".to_string(),
                    "Phone number is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "proof_of_address" {
            // TODO: Implement file upload
            profile_data.proof_of_address = "somefile".to_string();
            // If file upload fails, add an error to the context
            if profile_data.proof_of_address.is_empty() {
                context.insert(
                    "proof_of_address_error".to_string(),
                    "Proof of address is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
            // Handle file upload (e.g., save to cloud storage)
        }
    }
    if context.contains_key("errors") {
        return Err(context);
    }
    Ok((user_data, profile_data))
}

pub async fn register_loan(
    State(mut state): State<state::AppState>,
    multipart: Multipart,
) -> impl IntoResponse {
    // Validate form fields and handle file uploads
    let result = get_registration_data(multipart).await;
    if result.is_err() {
        let mut context = result.err().unwrap();
        context.insert("title".to_string(), "Robia Labs Ltd".to_string());
        log::warn!("Validation errors: {:?}", context);

        let res = render_template(&mut state.tera, "src/templates/index.html", &context);
        return Ok((
            get_headers().unwrap_or_else(|_| HeaderMap::new()),
            res.unwrap(),
        ));
    }
    let (user_data, mut profile_data) = result.unwrap();

    let mut context = std::collections::HashMap::new();
    context.insert("title".to_string(), "Robia Labs Ltd".to_string());

    // Create user
    match models::User::create(&state.pool, &user_data).await {
        Ok(user) => {
            log::info!("Created user with ID: {}", user.id);
            // Create user profile
            profile_data.user_id = user.id;
            match models::UserProfile::create(&state.pool, &profile_data).await {
                Ok(profile) => {
                    log::info!("Created user profile for user ID: {}", profile.user_id);
                    // Redirect to loan application page after successful registration
                    context.insert(
                    "success_popup".to_string(),
                    "Your loan application has been submitted, please check your email for further instructions."
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
                        "A user with this profile already exists.".to_string(),
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
                "A user with this profile already exists.".to_string(),
            );
        }
    }

    let result = render_template(&mut state.tera, "src/templates/index.html", &context);
    if result.is_err() {
        return Err(AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(result.err().unwrap()),
        ));
    }
    let content = result.unwrap();
    Ok((get_headers().unwrap_or_else(|_| HeaderMap::new()), content))
}

pub async fn loan_application(
    State(mut state): State<state::AppState>,
    auth::ExtractAuthenticationCode(_auth): auth::ExtractAuthenticationCode,
) -> impl IntoResponse {
    let context = std::collections::HashMap::new();

    let result = render_template(
        &mut state.tera,
        "src/templates/loan_application.html",
        &context,
    );
    if result.is_err() {
        return Err(AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(result.err().unwrap()),
        ));
    }
    let content = result.unwrap();

    Ok((get_headers().unwrap_or_else(|_| HeaderMap::new()), content))
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
        }
    }

    #[test]
    fn get_headers_sets_html_content_type() {
        let headers = get_headers().unwrap();
        assert_eq!(headers.get("content-type").unwrap(), "text/html");
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
