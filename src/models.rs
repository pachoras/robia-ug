use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};

#[derive(Clone, Debug, Deserialize)]
pub struct UserData {
    pub email: String,
    pub password_hash: String,
}

impl UserData {
    pub fn new() -> Self {
        UserData {
            email: String::new(),
            password_hash: String::new(),
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create new empty object
    pub fn new(email: String, password_hash: String) -> Self {
        User {
            id: 0, // This will be set by the database
            email,
            password_hash,
            is_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a new user. Returns the created user or an error if there's a database issue.
    pub async fn create(pool: &sqlx::PgPool, create_user: &UserData) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let res =
            sqlx::query_as("INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING *")
                .bind(&create_user.email)
                .bind(&create_user.password_hash)
                .fetch_one(&mut *tx)
                .await;
        if res.is_err() {
            tx.rollback().await?;
            return Err(res.err().unwrap());
        }
        tx.commit().await?;
        res
    }
    /// Deletes a user by its ID. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, id: i32) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot delete user with id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(())
    }
    /// Reads a user by its ID. Returns an error if not found or if there's a database issue.
    pub async fn read(pool: &sqlx::PgPool, id: i32) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as("SELECT * FROM users WHERE id = $1")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot read user with id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Updates a user by its ID. Returns the updated user or an error if there's a database issue.
    pub async fn update(pool: &sqlx::PgPool, id: i32, data: &User) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
                "UPDATE users SET email = $1, password_hash = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $3 RETURNING *",
            )
            .bind(&data.email)
            .bind(&data.password_hash)
            .bind(&id)
            .fetch_one(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot update user with id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserProfileData {
    pub user_id: i32,
    pub full_name: String,
    pub national_id: String,
    pub phone_number: String,
    pub proof_of_address: String,
}

impl UserProfileData {
    pub fn new() -> Self {
        UserProfileData {
            user_id: 0,
            full_name: String::new(),
            national_id: String::new(),
            phone_number: String::new(),
            proof_of_address: String::new(),
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: i32,
    pub user_id: i32,
    pub full_name: String,
    pub national_id: String,
    pub phone_number: String,
    pub proof_of_address: String,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserProfile {
    // Create new empty userprofile object
    pub fn new(
        user_id: i32,
        full_name: String,
        national_id: String,
        phone_number: String,
        proof_of_address: String,
    ) -> Self {
        UserProfile {
            id: 0, // This will be set by the database
            user_id,
            full_name,
            national_id,
            phone_number,
            proof_of_address,
            is_verified: false,
            created_at: Utc::now(),
            updated_at: Utc::now(), // This will be set by the database
        }
    }
    /// Creates a new user profile. Returns the created profile or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        profile: &UserProfileData,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as("INSERT INTO user_profiles (user_id, full_name, national_id, phone_number, proof_of_address) VALUES ($1, $2, $3, $4, $5) RETURNING *")
            .bind(&profile.user_id)
            .bind(&profile.full_name)
            .bind(&profile.national_id)
            .bind(&profile.phone_number)
            .bind(&profile.proof_of_address)
            .fetch_one(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!(
                "Cannot create user profile for user_id {}: ",
                profile.user_id
            );
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Reads a user profile by its user ID. Returns an error if not found or if there's a database issue.
    pub async fn read(pool: &sqlx::PgPool, user_id: i32) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as("SELECT * FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot read user profile for user_id {}: ", user_id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Updates a user profile by its user ID. Returns the updated profile or an error if there's a database issue.
    pub async fn update(
        pool: &sqlx::PgPool,
        user_id: i32,
        profile: &UserProfile,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as("UPDATE user_profiles SET full_name = $1, national_id = $2, phone_number = $3, proof_of_address = $4 WHERE user_id = $5 RETURNING *")
            .bind(&profile.full_name)
            .bind(&profile.national_id)
            .bind(&profile.phone_number)
            .bind(&profile.proof_of_address)
            .bind(&user_id)
            .fetch_one(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot update user profile for user_id {}: ", user_id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Deletes a user profile by its user ID. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, user_id: i32) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_optional(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot delete user profile for user_id {}: ", user_id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(())
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct UserAuthToken {
    pub token: String,
    pub user_id: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserAuthToken {
    /// Creates a new user auth token. Returns the created token or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &UserAuthToken,
    ) -> Result<UserAuthToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "INSERT INTO user_auth_tokens (token, user_id) VALUES ($1, $2) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!(
                "Cannot create user auth token for user_id {}: ",
                create_token.user_id
            );
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Deletes a user auth token by its token string. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, token: &String) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM user_auth_tokens WHERE token = $1")
            .bind(&token)
            .fetch_optional(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot delete user auth token for token {}: ", token);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(())
    }
    /// Reads a user auth token by its ID. Returns an error if not found or if there's a database issue.
    pub async fn read(pool: &sqlx::PgPool, id: &i32) -> Result<UserAuthToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result =
            sqlx::query_as::<_, UserAuthToken>("SELECT * FROM user_auth_tokens WHERE id = $1")
                .bind(&id)
                .fetch_one(&mut *tx)
                .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot read user auth token for id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Finds a user auth token by its token string. Returns an error if not found or if there's a database issue.
    pub async fn find_by_token(
        pool: &sqlx::PgPool,
        token: &String,
    ) -> Result<UserAuthToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result =
            sqlx::query_as::<_, UserAuthToken>("SELECT * FROM user_auth_tokens WHERE token = $1")
                .bind(&token)
                .fetch_one(&mut *tx)
                .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot find user auth token for token {}: ", token);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
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
