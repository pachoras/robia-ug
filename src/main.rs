mod api;
mod auth;
mod consts;
mod files;
mod forms;
mod mail;
mod middleware;
mod models;
mod renderer;
mod responses;
mod routes;
mod session;
mod state;
mod utils;
mod workflows;

use std::{collections::HashMap, sync::Arc};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use tokio::net::TcpListener;
use tokio::{self, sync::RwLock};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{self, EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    println!("Starting router...");
    // Open database connection and run migrations on startup
    let pool = models::connect_to_db()
        .await
        .unwrap_or_else(|e| panic!("Failed to connect to the database: {}", e));
    models::run_migrations(&pool)
        .await
        .unwrap_or_else(|_| panic!("Failed to run database migrations"));

    // Create the application state
    let tera = renderer::init_renderer();
    let s3_client = files::initialize_s3_client().await;
    let bucket = Arc::new(RwLock::new(HashMap::new()));
    let state = state::AppState {
        tera,
        pool,
        s3_client,
        rate_limit_bucket: bucket,
    };
    // Initialize the router with the application state
    let app = init_router(state);

    // Listen on all interfaces on port 8000
    let listener = TcpListener::bind("0.0.0.0:8000")
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to address: {}", e));
    println!(
        "listening on {}",
        listener
            .local_addr()
            .unwrap_or_else(|e| panic!("Cannot get local address: {}", e))
    );
    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| panic!("Failed to start server: {}", e));
}

fn init_router(state: state::AppState) -> Router {
    // 1. Initialize tracing + log bridging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    Router::new()
        .route("/", get(routes::index))
        .route("/register-loan", post(routes::register_loan_application))
        .route(
            "/register-provider",
            post(routes::register_provider_application),
        )
        .route("/verify-token/{token}", get(routes::verify_token))
        .route("/update-password", post(routes::update_password))
        .route("/login", get(routes::login_page))
        .route("/login", post(routes::handle_login))
        .route("/login-google", post(api::login_google))
        .route("/forgot-password", get(routes::forgot_password_page))
        .route("/forgot-password", post(routes::handle_forgot_password))
        .nest_service("/static", ServeDir::new("src/static"))
        .layer(DefaultBodyLimit::disable())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::default_rate_limit,
                ))
                .layer(
                    CorsLayer::new()
                        .allow_methods(tower_http::cors::Any)
                        .allow_headers(tower_http::cors::Any),
                )
                .layer(axum::middleware::from_fn(middleware::default_headers))
                .layer(axum::middleware::from_fn(middleware::security_headers))
                .layer(axum::middleware::from_fn(middleware::cache_control_headers))
                .layer(CompressionLayer::new()),
        )
        .with_state(state)
}
