use axum::response::{Html, IntoResponse, Response};
use reqwest::StatusCode;

use crate::{
    mail,
    renderer::{self, init_renderer},
};

#[derive(Debug)]
pub struct AppError {
    pub status_code: StatusCode,
    pub message: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let mut tera = init_renderer();
        let mut context = std::collections::HashMap::new();
        let mut response: Response;

        if self.status_code == StatusCode::NOT_FOUND {
            response = HtmlResponse {
                title: "404 Not Found".to_string(),
                path: "src/templates/404.html".to_string(),
                tera: &mut tera,
                context,
            }
            .into_response();
            *response.status_mut() = StatusCode::NOT_FOUND;
        } else if self.status_code == StatusCode::INTERNAL_SERVER_ERROR {
            response = HtmlResponse {
                title: "500 Internal Server Error".to_string(),
                path: "src/templates/500.html".to_string(),
                tera: &mut tera,
                context,
            }
            .into_response();
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            let error_message = self
                .message
                .clone()
                .unwrap_or_else(|| "Unknown error".to_string());
            log::error!("Internal server error: {}", error_message);
            mail::send_admin_error_email(&error_message).unwrap_or_else(|_| ());
        } else if self.status_code == StatusCode::UNAUTHORIZED {
            log::warn!(
                "Unauthorized access attempt: {}",
                self.message
                    .unwrap_or_else(|| "No additional info".to_string())
            );
            response = HtmlResponse {
                title: "Login".to_string(),
                path: "src/templates/login.html".to_string(),
                tera: &mut tera,
                context,
            }
            .into_response();
            *response.status_mut() = StatusCode::UNAUTHORIZED;
        } else if self.status_code == StatusCode::BAD_REQUEST {
            let error_message = self.message.unwrap_or_else(|| "Bad request".to_string());
            log::error!("Submission error: {}", error_message);
            context.insert("error_popup".to_string(), error_message);
            response = HtmlResponse {
                title: "Error".to_string(),
                path: "src/templates/index.html".to_string(),
                tera: &mut tera,
                context,
            }
            .into_response();
            *response.status_mut() = StatusCode::BAD_REQUEST;
        } else if self.status_code == StatusCode::TOO_MANY_REQUESTS {
            response = Html("Too many requests. Please try again later").into_response();
            *response.status_mut() = self.status_code;
        } else {
            response = HtmlResponse {
                title: "Error".to_string(),
                path: "src/templates/500.html".to_string(),
                tera: &mut tera,
                context,
            }
            .into_response();
            *response.status_mut() = self.status_code;
        }
        response
    }
}

pub struct HtmlResponse<'a> {
    pub title: String,
    pub path: String,
    pub tera: &'a mut tera::Tera,
    pub context: std::collections::HashMap<String, String>,
}

impl<'a> IntoResponse for HtmlResponse<'a> {
    fn into_response(mut self) -> Response {
        self.context.insert("title".to_string(), self.title);
        let res = renderer::render_template(&mut self.tera, &self.path, &self.context);
        Html(res.map_err(|e| {
            log::error!("Template rendering error: {}", e);
            return AppError {
                message: Some("Template rendering error:".to_string()),
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            };
        }))
        .into_response()
    }
}

pub struct SuccessPopupResponse<'a> {
    pub message: String,
    pub tera: &'a mut tera::Tera,
    pub path: &'static str,
    pub context: std::collections::HashMap<String, String>,
}

impl<'a> IntoResponse for SuccessPopupResponse<'a> {
    fn into_response(mut self) -> Response {
        self.context
            .insert("success_popup".to_string(), self.message);
        HtmlResponse {
            title: "Success".to_string(),
            path: self.path.to_string(),
            tera: &mut self.tera,
            context: self.context,
        }
        .into_response()
    }
}

pub struct ErrorPopupResponse<'a> {
    pub message: String,
    pub tera: &'a mut tera::Tera,
    pub path: &'static str,
    pub context: std::collections::HashMap<String, String>,
}

impl<'a> IntoResponse for ErrorPopupResponse<'a> {
    fn into_response(mut self) -> Response {
        self.context.insert("error_popup".to_string(), self.message);
        HtmlResponse {
            title: "Error".to_string(),
            path: self.path.to_string(),
            tera: &mut self.tera,
            context: self.context,
        }
        .into_response()
    }
}
