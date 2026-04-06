mod auth;
mod files;
mod forms;
mod mail;
mod middleware;
mod models;
mod renderer;
mod routes;
mod session;
mod state;
mod utils;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use tokio;
use tokio::net::TcpListener;
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
    let pool = models::connect_to_db().await.unwrap();
    models::run_migrations(&pool).await.unwrap();

    // Create the application state
    let tera = renderer::init_renderer();
    let s3_client = files::initialize_s3_client().await;
    let state = state::AppState {
        tera,
        pool,
        s3_client,
    };
    let app = init_router(state);

    // Listen on all interfaces on port 8000
    let listener = TcpListener::bind("0.0.0.0:8000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn init_router(state: state::AppState) -> Router {
    // 1. Initialize tracing + log bridging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    Router::new()
        .route("/", get(routes::index))
        .route("/register-loan", post(routes::submit_loan_application))
        .route("/login", get(routes::login_page))
        .route("/login-google", post(routes::login_google))
        .nest_service("/static", ServeDir::new("src/static"))
        .with_state(state)
        .layer(DefaultBodyLimit::disable())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
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
}
