mod auth;
mod models;
mod renderer;
mod routes;
mod state;
mod utils;

use axum::{Router, routing::get};
use routes::index;
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
    let state = state::AppState {
        tera: tera,
        pool: pool,
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
        .route("/", get(index))
        .route("/app", get(routes::loan_application))
        .nest_service("/static", ServeDir::new("src/static"))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_methods(tower_http::cors::Any)
                        .allow_headers(tower_http::cors::Any),
                )
                .layer(CompressionLayer::new()),
        )
}
