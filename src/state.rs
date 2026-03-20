// Application state
#[derive(Clone)]
pub struct AppState {
    pub tera: tera::Tera,
    pub pool: sqlx::PgPool,
}
