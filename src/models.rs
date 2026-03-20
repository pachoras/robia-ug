use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Deserialize)]
pub struct UserData {
    pub username: String,
    pub email: String,
    pub password_hash: String,
}

impl User {
    pub fn new(username: String, email: String, password_hash: String) -> Self {
        User {
            id: 0,
            username,
            email,
            password_hash,
            created_at: 0,
            updated_at: 0,
        }
    }
    async fn create(pool: &sqlx::PgPool, create_user: &UserData) -> Result<User, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_user.username)
        .bind(&create_user.email)
        .bind(&create_user.password_hash)
        .fetch_one(pool)
        .await
    }
    async fn delete(pool: &sqlx::PgPool, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&id)
            .fetch_optional(pool)
            .await?;
        Ok(())
    }
    async fn read(pool: &sqlx::PgPool, id: i32) -> Result<User, sqlx::Error> {
        sqlx::query_as("SELECT * FROM users WHERE id = $1")
            .bind(&id)
            .fetch_one(pool)
            .await
    }
    async fn update(pool: &sqlx::PgPool, id: i32, data: &User) -> Result<User, sqlx::Error> {
        sqlx::query_as(
                "UPDATE users SET username = $1, email = $2, password_hash = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $4 RETURNING *",
            )
            .bind(&data.username)
            .bind(&data.email)
            .bind(&data.password_hash)
            .bind(&id)
            .fetch_one(pool)
            .await
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct UserAuthToken {
    pub token: String,
    pub user_id: i32,
    pub created_at: i32,
    pub updated_at: i32,
}

impl UserAuthToken {
    pub fn new(token: String, user_id: i32) -> Self {
        UserAuthToken {
            token,
            user_id,
            created_at: 0,
            updated_at: 0,
        }
    }
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &UserAuthToken,
    ) -> Result<UserAuthToken, sqlx::Error> {
        sqlx::query_as("INSERT INTO user_auth_tokens (token, user_id) VALUES ($1, $2) RETURNING *")
            .bind(&create_token.token)
            .bind(&create_token.user_id)
            .fetch_one(pool)
            .await
    }
    pub async fn delete(pool: &sqlx::PgPool, token: &String) -> Result<(), sqlx::Error> {
        sqlx::query_as::<_, UserAuthToken>("DELETE FROM user_auth_tokens WHERE token = $1")
            .bind(&token)
            .fetch_optional(pool)
            .await?;
        Ok(())
    }
    pub async fn read(pool: &sqlx::PgPool, id: &i32) -> Result<UserAuthToken, sqlx::Error> {
        sqlx::query_as::<_, UserAuthToken>("SELECT * FROM user_auth_tokens WHERE id = $1")
            .bind(&id)
            .fetch_one(pool)
            .await
    }
    pub async fn find_by_token(
        pool: &sqlx::PgPool,
        token: &String,
    ) -> Result<UserAuthToken, sqlx::Error> {
        sqlx::query_as::<_, UserAuthToken>("SELECT * FROM user_auth_tokens WHERE token = $1")
            .bind(&token)
            .fetch_one(pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_new_sets_fields() {
        let u = User::new("alice".into(), "alice@example.com".into(), "hash123".into());
        assert_eq!(u.username, "alice");
        assert_eq!(u.email, "alice@example.com");
        assert_eq!(u.password_hash, "hash123");
    }

    #[test]
    fn user_new_sets_default_id_and_timestamps() {
        let u = User::new("bob".into(), "bob@example.com".into(), "h".into());
        assert_eq!(u.id, 0);
        assert_eq!(u.created_at, 0);
        assert_eq!(u.updated_at, 0);
    }

    #[test]
    fn user_auth_token_new_sets_fields() {
        let t = UserAuthToken::new("tok123".into(), 42);
        assert_eq!(t.token, "tok123");
        assert_eq!(t.user_id, 42);
    }

    #[test]
    fn user_auth_token_new_sets_default_timestamps() {
        let t = UserAuthToken::new("tok".into(), 1);
        assert_eq!(t.created_at, 0);
        assert_eq!(t.updated_at, 0);
    }
}

pub async fn connect_to_db() -> Result<sqlx::PgPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    sqlx::PgPool::connect(&database_url).await
}

pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
