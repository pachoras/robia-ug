use crate::auth;
use crate::renderer::{init_renderer, render_template};
use crate::state::AppState;
use crate::utils;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

fn get_headers() -> Result<HeaderMap, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/html".parse()?);
    Ok(headers)
}

pub async fn index(State(mut state): State<AppState>) -> impl IntoResponse {
    let mut context = std::collections::HashMap::new();
    let static_css_path = utils::generate_cache_busted_css_path().unwrap();
    context.insert("static_path".to_string(), static_css_path);
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

pub async fn loan_application(
    auth::ExtractAuthenticationCode(_auth): auth::ExtractAuthenticationCode,
) -> impl IntoResponse {
    let mut tera = init_renderer();
    let mut context = std::collections::HashMap::new();
    let static_css_path = utils::generate_cache_busted_css_path().unwrap();
    context.insert("static_path".to_string(), static_css_path);

    let result = render_template(&mut tera, "src/templates/loan_application.html", &context);
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
        let static_css_path = utils::generate_cache_busted_css_path().unwrap();
        let mut response = (StatusCode::INTERNAL_SERVER_ERROR, "").into_response();

        if self.0 == StatusCode::NOT_FOUND {
            context.insert("title".to_string(), "404 Not Found".to_string());
            context.insert("static_path".to_string(), static_css_path);
            let body = render_template(&mut tera, "src/templates/404.html", &context);
            response = (StatusCode::NOT_FOUND, body).into_response();
        } else if self.0 == StatusCode::INTERNAL_SERVER_ERROR {
            context.insert("title".to_string(), "500 Internal Server Error".to_string());
            context.insert("static_path".to_string(), static_css_path);
            log::error!(
                "Internal server error: {}",
                self.1.unwrap_or_else(|| "Unknown error".to_string())
            );
            let body = render_template(&mut tera, "src/templates/500.html", &context);
            response = (StatusCode::INTERNAL_SERVER_ERROR, body).into_response();
        } else if self.0 == StatusCode::UNAUTHORIZED {
            context.insert("static_path".to_string(), static_css_path);
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
