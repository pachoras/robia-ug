use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};

use crate::mail::send_email;
use crate::{files, utils};

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
    pub async fn find(pool: &sqlx::PgPool, id: i32) -> Result<User, sqlx::Error> {
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
    pub async fn create_with_email(
        pool: &sqlx::PgPool,
        email: &String,
    ) -> Result<User, sqlx::Error> {
        let _email = email.clone();
        let user = User::create(
            pool,
            &UserData {
                email: email.clone(),
                password_hash: "".to_string(), // User will set password later
            },
        )
        .await?;
        // Send verification email
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
        let link = format!(
            "https://{}/verify-registration?user_id={}",
            hostname, user.id
        );
        let body = format!(
            r#"Your loan application has been received, please click the link below to complete your 
            registration and view your loan application status. If you cannot click the link, please copy and 
            paste the following URL into your browser:    {}"#,
            link
        );
        // Send email in background task to avoid blocking the main thread
        tokio::spawn(async move {
            send_email(
                "Robia Labs <no-reply@robialabs.com>",
                &_email,
                "Welcome to Robia Loans",
                &body,
                &link,
                "Verify email",
            )
            .await
            .unwrap();
        });
        Ok(user)
    }
        /// Finds a user profile by its email. Returns an error if not found or if there's a database issue.
    pub async fn find_by_email(
        pool: &sqlx::PgPool,
        email: &String,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as("SELECT * FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&mut *tx)
            .await
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserProfileData {
    pub user_id: i32,
    pub full_name: String,
    pub national_id: String,
    pub phone_number: String,
    pub proof_of_address: Vec<u8>,
    pub proof_of_address_file_format: String,
    pub national_id_back: Vec<u8>,
    pub national_id_back_file_format: String,
    pub national_id_front: Vec<u8>,
    pub national_id_front_file_format: String,
    pub google_id: Option<String>,
}

impl UserProfileData {
    pub fn new() -> Self {
        UserProfileData {
            user_id: 0,
            full_name: String::new(),
            national_id: String::new(),
            phone_number: String::new(),
            proof_of_address: Vec::new(),
            proof_of_address_file_format: String::new(),
            national_id_back: Vec::new(),
            national_id_back_file_format: String::new(),
            national_id_front: Vec::new(),
            national_id_front_file_format: String::new(),
            google_id: None,
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: i32,
    pub user_id: i32,
    pub full_name: String,
    pub national_id_back: String,
    pub national_id_front: String,
    pub phone_number: String,
    pub proof_of_address: String,
    pub is_verified: bool,
    pub google_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserProfile {
    // Create new empty userprofile object
    pub fn new(
        user_id: i32,
        full_name: String,
        phone_number: String,
        proof_of_address: String,
        national_id_front: String,
        national_id_back: String,
    ) -> Self {
        UserProfile {
            id: 0, // This will be set by the database
            user_id,
            full_name,
            national_id_back,
            national_id_front,
            phone_number,
            proof_of_address,
            is_verified: false,
            google_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(), // This will be set by the database
        }
    }
    /// Creates a new user profile. Returns the created profile or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        s3_client: &aws_sdk_s3::Client,
        profile: &UserProfileData,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        // Get correct file names for proof of address and national ID files based on user ID and file formats
        let file_name = utils::get_proof_of_address_path(
            &profile.user_id,
            &profile.proof_of_address_file_format,
        );
        let national_id_front_file_name = utils::get_national_id_front_path(
            &profile.user_id,
            &profile.national_id_front_file_format,
        );
        let national_id_back_file_name = utils::get_national_id_back_path(
            &profile.user_id,
            &profile.national_id_back_file_format,
        );

        let result = sqlx::query_as("INSERT INTO user_profiles (user_id, full_name, phone_number, proof_of_address, national_id_front, national_id_back, google_id) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *")
            .bind(&profile.user_id)
            .bind(&profile.full_name)
            .bind(&profile.phone_number)
            .bind(&file_name)
            .bind(&national_id_front_file_name)
            .bind(&national_id_back_file_name)
            .bind(&profile.google_id)
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
        // Upload proof of address file to cloud storage
        let data = Bytes::from(profile.proof_of_address.clone());
        let _client = s3_client.clone();

        // Upload national ID front file to cloud storage
        let national_id_front_file_name = format!(
            "national_id_front_{}.{}",
            profile.user_id, profile.national_id_front_file_format
        );
        let national_id_front_data = Bytes::from(profile.national_id_front.clone());
        let _client = s3_client.clone();

        // Upload national ID back file to cloud storage
        let national_id_back_file_name = format!(
            "national_id_back_{}.{}",
            profile.user_id, profile.national_id_back_file_format
        );
        let national_id_back_data = Bytes::from(profile.national_id_back.clone());
        let _client = s3_client.clone();

        // Upload files in background task to avoid blocking the main thread
        tokio::spawn(async move {
            files::upload_file_to_s3(&_client, &file_name, data)
                .await
                .unwrap_or(());
            files::upload_file_to_s3(
                &_client,
                &national_id_front_file_name,
                national_id_front_data,
            )
            .await
            .unwrap_or(());
            files::upload_file_to_s3(&_client, &national_id_back_file_name, national_id_back_data)
                .await
                .unwrap_or(());
        });
        Ok(result.unwrap())
    }
    /// Reads a user profile by its user ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, user_id: i32) -> Result<UserProfile, sqlx::Error> {
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
        let result = sqlx::query_as("UPDATE user_profiles SET full_name = $1, phone_number = $2, proof_of_address = $3, national_id_front = $4, national_id_back = $5 WHERE user_id = $6 RETURNING *")
            .bind(&profile.full_name)
            .bind(&profile.phone_number)
            .bind(&profile.proof_of_address)
            .bind(&profile.national_id_front)
            .bind(&profile.national_id_back)
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
    /// Finds a user profile by its phone number. Returns an error if not found or if there's a database issue.
    pub async fn find_by_phone_number(
        pool: &sqlx::PgPool,
        phone_number: &String,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as("SELECT * FROM user_profiles WHERE phone_number = $1")
            .bind(&phone_number)
            .fetch_one(&mut *tx)
            .await
    }

}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct UserAuthToken {
    pub token: String,
    pub user_id: i32,
    pub app: String,
    pub is_used: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserAuthToken {
    pub fn new(user_id: i32, app: String, token: String) -> Self {
        UserAuthToken {
            token,
            user_id,
            app,
            is_used: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    /// Creates a new user auth token. Returns the created token or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &UserAuthToken,
    ) -> Result<UserAuthToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "INSERT INTO user_auth_tokens (token, user_id, app, is_used) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .bind(&create_token.app)
        .bind(&create_token.is_used)
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
    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<UserAuthToken, sqlx::Error> {
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

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct RegistrationToken {
    pub token: String,
    pub user_id: i32,
    pub app: String,
    pub is_used: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RegistrationToken {
    /// Creates a new user auth token. Returns the created token or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &RegistrationToken,
    ) -> Result<RegistrationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "INSERT INTO registration_tokens (token, user_id, app) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .bind(&create_token.app)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!(
                "Cannot create registration token for user_id {}: ",
                create_token.user_id
            );
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Deletes a registration token by its token string. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, token: &String) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM registration_tokens WHERE token = $1")
            .bind(&token)
            .fetch_optional(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot delete registration token for token {}: ", token);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(())
    }
    /// Reads a registration token by its ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<RegistrationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, RegistrationToken>(
            "SELECT * FROM registration_tokens WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot read registration token for id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Finds a registration token by its token string. Returns an error if not found or if there's a database issue.
    pub async fn find_by_token(
        pool: &sqlx::PgPool,
        token: &String,
    ) -> Result<RegistrationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, RegistrationToken>(
            "SELECT * FROM registration_tokens WHERE token = $1",
        )
        .bind(&token)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot find registration token for token {}: ", token);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct PasswordResetToken {
    pub token: String,
    pub user_id: i32,
    pub is_used: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PasswordResetToken {
    pub fn new(user_id: i32, token: String) -> Self {
        PasswordResetToken {
            token,
            user_id,
            is_used: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    /// Creates a new password reset token. Returns the created token or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &PasswordResetToken,
    ) -> Result<PasswordResetToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "INSERT INTO password_reset_tokens (token, user_id, is_used) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .bind(&create_token.is_used)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!(
                "Cannot create password reset token for user_id {}: ",
                create_token.user_id
            );
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Deletes a password reset token by its token string. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, token: &String) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM password_reset_tokens WHERE token = $1")
            .bind(&token)
            .fetch_optional(&mut *tx)
            .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot delete password reset token for token {}: ", token);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(())
    }
    /// Reads a password reset token by its ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<PasswordResetToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, PasswordResetToken>(
            "SELECT * FROM password_reset_tokens WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot read password reset token for id {}: ", id);
            return Err(result.err().unwrap());
        }
        tx.commit().await?;
        Ok(result.unwrap())
    }
    /// Finds a password reset token by its token string. Returns an error if not found or if there's a database issue.
    pub async fn find_by_token(
        pool: &sqlx::PgPool,
        token: &String,
    ) -> Result<PasswordResetToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, PasswordResetToken>(
            "SELECT * FROM password_reset_tokens WHERE token = $1",
        )
        .bind(&token)
        .fetch_one(&mut *tx)
        .await;
        if result.is_err() {
            tx.rollback().await?;
            log::error!("Cannot find password reset token for token {}: ", token);
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
