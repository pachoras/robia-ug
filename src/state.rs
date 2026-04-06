// Application state
#[derive(Clone)]
pub struct AppState {
    pub tera: tera::Tera,
    pub pool: sqlx::PgPool,
    pub s3_client: aws_sdk_s3::Client,
}
