use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::mail::send_email;
use crate::{files, forms, utils};

/// Helper function to commit a transaction if the result is Ok, or rollback if it's an Err. Returns the unwrapped result or error.
pub async fn commit_else_rollback<T>(
    tx: sqlx::Transaction<'_, sqlx::Postgres>,
    result: Result<T, sqlx::Error>,
) -> Result<T, sqlx::Error> {
    match result {
        Ok(val) => {
            tx.commit().await?;
            Ok(val)
        }
        Err(e) => {
            log::error!("Database transaction failed: {}", e);
            tx.rollback().await?;
            Err(e)
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Create new empty object
    pub fn new(email: String, password: String) -> Self {
        let salt = utils::generate_random_string(16);
        let password_hash = utils::get_password_hash(&password, &salt);
        User {
            id: 0, // This will be set by the database
            email,
            password_hash,
            salt,
            is_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a new user. Returns the created user or an error if there's a database issue.
    pub async fn create(pool: &sqlx::PgPool, create_user: &User) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let res = sqlx::query_as::<_, User>(
            "INSERT INTO users (email, password_hash, salt) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_user.email)
        .bind(&create_user.password_hash)
        .bind(&create_user.salt)
        .fetch_one(&mut *tx)
        .await;
        commit_else_rollback(tx, res).await
    }

    /// Deletes a user by its ID. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, id: i32) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await;
        commit_else_rollback(tx, result).await?;
        Ok(())
    }
    /// Reads a user by its ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: i32) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await
    }
    /// Finds a user profile by its email. Returns an error if not found or if there's a database issue.
    pub async fn find_by_email(pool: &sqlx::PgPool, email: &String) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&mut *tx)
            .await
    }
    /// Updates a user by its ID. Returns the updated user or an error if there's a database issue.
    pub async fn update(pool: &sqlx::PgPool, id: i32, data: &User) -> Result<User, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, User>(
                "UPDATE users SET email = $1, password_hash = $2, salt = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $4 RETURNING *",
            )
            .bind(&data.email)
            .bind(&data.password_hash)
            .bind(&data.salt)
            .bind(&id)
            .fetch_one(&mut *tx)
            .await;
        commit_else_rollback(tx, result).await
    }
    /// Creates a new loan applicant user and sends them a verification email. Returns the created user or an error if there's a database issue.
    pub async fn create_loan_applicant(
        pool: &sqlx::PgPool,
        email: &String,
    ) -> Result<User, sqlx::Error> {
        let user = User::new(email.clone(), utils::generate_random_string(12));
        match User::create(pool, &user).await {
            Ok(created_user) => {
                // Also create registration token for user
                ApplicationToken::create_registration_token(pool, &created_user).await?;
                Ok(created_user)
            }
            Err(e) => return Err(e),
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
        profile: &forms::UserProfileData,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        // Get correct file names for proof of address and national ID files based on user ID and file formats
        let proof_of_address_file_name = files::get_proof_of_address_path(
            &profile.user_id,
            &profile.proof_of_address_file_format,
        );
        let national_id_front_file_name = files::get_national_id_front_path(
            &profile.user_id,
            &profile.national_id_front_file_format,
        );
        let national_id_back_file_name = files::get_national_id_back_path(
            &profile.user_id,
            &profile.national_id_back_file_format,
        );

        let mut result: Result<UserProfile, sqlx::Error> = sqlx::query_as("INSERT INTO user_profiles (user_id, full_name, phone_number, proof_of_address, national_id_front, national_id_back, google_id) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *")
            .bind(&profile.user_id)
            .bind(&profile.full_name)
            .bind(&profile.phone_number)
            .bind(&proof_of_address_file_name)
            .bind(&national_id_front_file_name)
            .bind(&national_id_back_file_name)
            .bind(&profile.google_id)
            .fetch_one(&mut *tx)
            .await;
        result = commit_else_rollback(tx, result).await;

        // Get additional files if they exist
        let mut data_map = HashMap::new();
        if let Some(additional_files) = &profile.additional_files {
            for (file_name, data) in additional_files.iter() {
                let file_path = files::get_additional_file_path(&profile.user_id, file_name);
                let data = Bytes::from(data.clone());
                data_map.insert(file_path, data);
            }
        }

        // Upload proof of address file to cloud storage
        let proof_of_address_data = Bytes::from(profile.proof_of_address.clone());
        data_map.insert(proof_of_address_file_name.clone(), proof_of_address_data);

        // Upload national ID front file to cloud storage
        let national_id_front_data = Bytes::from(profile.national_id_front.clone());
        data_map.insert(national_id_front_file_name.clone(), national_id_front_data);

        // Upload national ID back file to cloud storage
        let national_id_back_data = Bytes::from(profile.national_id_back.clone());
        data_map.insert(national_id_back_file_name.clone(), national_id_back_data);

        // Upload files in background task to avoid blocking the main thread
        let _client = s3_client.clone();

        tokio::spawn(async move {
            // Upload additional files to S3
            for (file_name, data) in data_map.iter() {
                match files::upload_file_to_s3(&_client, file_name, data.to_owned()).await {
                    Ok(_) => {
                        log::info!("Uploaded file {}", file_name,);
                    }
                    Err(e) => {
                        log::error!("Error uploading file {}: {}", file_name, e);
                    } // Continue with other files even if one fails
                }
            }
        });
        Ok(result.unwrap())
    }
    /// Reads a user profile by its user ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, user_id: i32) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as("SELECT * FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(&mut *tx)
            .await
    }
    /// Updates a user profile by its user ID. Returns the updated profile or an error if there's a database issue.
    pub async fn update(
        pool: &sqlx::PgPool,
        profile: &UserProfile,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as("UPDATE user_profiles SET full_name = $1, phone_number = $2, proof_of_address = $3, national_id_front = $4, national_id_back = $5 WHERE user_id = $6 RETURNING *")
            .bind(&profile.full_name)
            .bind(&profile.phone_number)
            .bind(&profile.proof_of_address)
            .bind(&profile.national_id_front)
            .bind(&profile.national_id_back)
            .bind(&profile.user_id)
            .fetch_one(&mut *tx)
            .await
    }
    /// Deletes a user profile by its user ID. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, user_id: i32) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_optional(&mut *tx)
            .await;
        commit_else_rollback(tx, result).await?;
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
    /// Finds a user profile by its email. Returns an error if not found or if there's a database issue.
    pub async fn find_by_email(
        pool: &sqlx::PgPool,
        email: &String,
    ) -> Result<UserProfile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE email = $1")
            .bind(&email)
            .fetch_one(&mut *tx)
            .await
    }
}
pub enum TokenTypeVariants {
    PasswordReset,
    LoansEmailVerification,
    ProEmailVerification,
    LoansAuthentication,
    ProAuthentication,
    AdminAuthentication,
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct ApplicationToken {
    pub id: i32,
    pub user_id: i32,
    pub token: String,
    pub token_type: i32,
    pub is_used: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ApplicationToken {
    pub fn new(user_id: i32, token_type: TokenTypeVariants, token: String) -> Self {
        let variant_type = match token_type {
            TokenTypeVariants::PasswordReset => 0,
            TokenTypeVariants::LoansEmailVerification => 1,
            TokenTypeVariants::ProEmailVerification => 2,
            TokenTypeVariants::LoansAuthentication => 3,
            TokenTypeVariants::ProAuthentication => 4,
            TokenTypeVariants::AdminAuthentication => 5,
        };
        ApplicationToken {
            id: 0, // This will be set by the database
            user_id,
            token,
            token_type: variant_type,
            is_used: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    /// Creates a new application token. Returns the created token or an error if there's a database issue.
    pub async fn create(
        pool: &sqlx::PgPool,
        create_token: &ApplicationToken,
    ) -> Result<ApplicationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, ApplicationToken>(
            "INSERT INTO application_tokens (token, user_id, token_type) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .bind(&create_token.token_type)
        .fetch_one(&mut *tx)
        .await;
        commit_else_rollback(tx, result).await
    }
    /// Deletes a registration token by its token string. Returns an error if there's a database issue.
    pub async fn delete(pool: &sqlx::PgPool, token: &String) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM application_tokens WHERE token = $1")
            .bind(&token)
            .fetch_optional(&mut *tx)
            .await;
        commit_else_rollback(tx, result).await?;
        Ok(())
    }
    /// Marks a registration token as used by its ID. Returns the updated token or an error if there's a database issue.
    pub async fn set_used(pool: &sqlx::PgPool, id: &i32) -> Result<ApplicationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as::<_, ApplicationToken>(
            "UPDATE application_tokens SET is_used = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 RETURNING *",
        )
        .bind(true)
        .bind(&id)
        .fetch_one(&mut *tx)
        .await;
        commit_else_rollback(tx, result).await
    }
    /// Returns the type of the token as a TokenTypeVariants enum. Returns an error if the token type is invalid.
    pub fn get_token_variant(&self) -> Result<TokenTypeVariants, String> {
        match self.token_type {
            0 => Ok(TokenTypeVariants::PasswordReset),
            1 => Ok(TokenTypeVariants::LoansEmailVerification),
            2 => Ok(TokenTypeVariants::ProEmailVerification),
            3 => Ok(TokenTypeVariants::LoansAuthentication),
            4 => Ok(TokenTypeVariants::ProAuthentication),
            5 => Ok(TokenTypeVariants::AdminAuthentication),
            _ => Err(format!("Invalid token type: {}", self.token_type)),
        }
    }
    /// Reads a registration token by its ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<ApplicationToken, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, ApplicationToken>("SELECT * FROM application_tokens WHERE id = $1")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await
    }
    /// Finds a registration token by its token string. Returns an error if not found or if there's a database issue.
    pub async fn find_by_token(
        pool: &sqlx::PgPool,
        token: &String,
    ) -> Result<ApplicationToken, sqlx::Error> {
        log::info!("Finding token: {}", token);
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, ApplicationToken>("SELECT * FROM application_tokens WHERE token = $1")
            .bind(&token)
            .fetch_one(&mut *tx)
            .await
    }
    /// Finds any token by user ID and token type. Returns an error if not found or if there's a database issue.
    pub async fn find_any_by_user_id_and_type(
        pool: &sqlx::PgPool,
        user_id: i32,
        token_type: TokenTypeVariants,
    ) -> Result<ApplicationToken, sqlx::Error> {
        let variant_type = match token_type {
            TokenTypeVariants::PasswordReset => 0,
            TokenTypeVariants::LoansEmailVerification => 1,
            TokenTypeVariants::ProEmailVerification => 2,
            TokenTypeVariants::LoansAuthentication => 3,
            TokenTypeVariants::ProAuthentication => 4,
            TokenTypeVariants::AdminAuthentication => 5,
        };
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, ApplicationToken>(
            "SELECT * FROM application_tokens WHERE user_id = $1 AND token_type = $2",
        )
        .bind(&user_id)
        .bind(&variant_type)
        .fetch_one(&mut *tx)
        .await
    }
    /// Verifies a registration token by its token string.
    pub async fn verify(self, pool: &sqlx::PgPool) -> Result<ApplicationToken, sqlx::Error> {
        let app_token = ApplicationToken::find_by_token(pool, &self.token).await?;
        // Check if token has been used
        if app_token.is_used {
            log::error!("Registration token {} has already been used", self.token);
            return Err(sqlx::Error::RowNotFound);
        }
        // Check if token is expired (valid for 24 hours)
        let now = Utc::now();
        if now.signed_duration_since(app_token.created_at).num_hours() >= 24 {
            log::error!("Registration token {} has expired", self.token);
            return Err(sqlx::Error::RowNotFound);
        }
        Ok(app_token)
    }
    /// Creates a new registration token for a user. Returns the created token or an error if there's a database issue.
    pub async fn create_registration_token(
        pool: &sqlx::PgPool,
        user: &User,
    ) -> Result<ApplicationToken, sqlx::Error> {
        let create_token = ApplicationToken::new(
            user.id,
            TokenTypeVariants::LoansEmailVerification,
            utils::generate_random_string(64),
        );
        match ApplicationToken::create(&pool, &create_token).await {
            Ok(token) => {
                // Send verification email
                let hostname =
                    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost:8000".to_string());
                let proto: String = if hostname.contains("localhost") {
                    "http".to_string()
                } else {
                    "https".to_string()
                };
                let link = format!(
                    "{}://{}/verify-token/{}",
                    proto, hostname, create_token.token
                );
                let body = format!(
                    r#"Your loan application has been received, please click the link below to complete your 
                    registration and view your loan application status. 
                    
                    If you cannot click the link, please copy and paste the following URL into your browser:    {}"#,
                    link
                );
                // Send email in background task to avoid blocking the main thread
                let user = user.clone();
                tokio::spawn(async move {
                    send_email(
                        "Robia Labs <no-reply@robialabs.com>",
                        &user.email,
                        "Welcome to Robia Loans",
                        &body,
                        &link,
                        "Verify email",
                    )
                    .await
                    .unwrap();
                });
                Ok(token)
            }
            Err(e) => return Err(e),
        }
    }
    /// Creates a new password reset token for a user. Returns the created token or an error if there's a database issue.
    pub async fn create_password_reset_token(
        pool: &sqlx::PgPool,
        user: &User,
    ) -> Result<ApplicationToken, sqlx::Error> {
        let create_token = ApplicationToken::new(
            user.id,
            TokenTypeVariants::PasswordReset,
            utils::generate_random_string(64),
        );
        // First delete any existing password reset tokens for the user to prevent multiple valid tokens at the same time
        match ApplicationToken::find_any_by_user_id_and_type(
            pool,
            user.id,
            TokenTypeVariants::PasswordReset,
        )
        .await
        {
            Ok(existing_token) => {
                log::info!(
                    "Existing password reset token found for user with email {}, deleting it",
                    user.email
                );
                match ApplicationToken::delete(pool, &existing_token.token).await {
                    Ok(_) => {
                        log::info!(
                            "Deleted existing password reset token for user with email {}",
                            user.email
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "Error deleting existing password reset token for user with email {}: {}",
                            user.email,
                            e
                        );
                        return Err(e);
                    }
                }
            }
            Err(sqlx::Error::RowNotFound) => {
                // No existing token found, continue with creating new one
            }
            Err(e) => {
                log::error!(
                    "Error checking for existing password reset token for user with email {}: {}",
                    user.email,
                    e
                );
                return Err(e);
            }
        }

        match ApplicationToken::create(&pool, &create_token).await {
            Ok(token) => {
                // Send verification email
                let hostname =
                    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
                let proto: String = if hostname == "localhost" {
                    "http".to_string()
                } else {
                    "https".to_string()
                };
                let link = format!(
                    "{}://{}/verify-token/{}",
                    proto, hostname, create_token.token
                );
                let body = format!(
                    r#"You recently requested a password reset. Please click the link below to reset your password.

                    If you did not request a password reset, please ignore this email or reply to let us know. 
                    This password reset link is only valid for the next 24 hours.
                    
                    If you cannot click the link, please copy and 
                    paste the following URL into your browser:    {}"#,
                    link
                );
                // Send email in background task to avoid blocking the main thread
                let user = user.clone();
                tokio::spawn(async move {
                    send_email(
                        "Robia Labs <no-reply@robialabs.com>",
                        &user.email,
                        "Password Reset Request",
                        &body,
                        &link,
                        "Reset Password",
                    )
                    .await
                    .unwrap();
                });
                Ok(token)
            }
            Err(e) => return Err(e),
        }
    }
    /// Creates a new authentication token for a user based on the application variant. Returns the created token or an error if there's a database issue.
    pub async fn create_auth_token(
        pool: &sqlx::PgPool,
        user_id: i32,
        token_variant: TokenTypeVariants,
    ) -> Result<ApplicationToken, sqlx::Error> {
        let create_token: ApplicationToken;
        match token_variant {
            TokenTypeVariants::LoansAuthentication => {
                create_token = ApplicationToken::new(
                    user_id,
                    TokenTypeVariants::LoansAuthentication,
                    utils::generate_random_string(64),
                );
            }
            TokenTypeVariants::ProAuthentication => {
                create_token = ApplicationToken::new(
                    user_id,
                    TokenTypeVariants::ProAuthentication,
                    utils::generate_random_string(64),
                );
            }
            TokenTypeVariants::AdminAuthentication => {
                create_token = ApplicationToken::new(
                    user_id,
                    TokenTypeVariants::AdminAuthentication,
                    utils::generate_random_string(64),
                );
            }
            _ => {
                log::error!("Invalid application variant specified for auth token creation");
                return Err(sqlx::Error::RowNotFound);
            }
        };
        match ApplicationToken::create(&pool, &create_token).await {
            Ok(token) => Ok(token),
            Err(e) => return Err(e),
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct AdditionalFile {
    pub id: i32,
    pub user_id: i32,
    pub file_name: String,
    pub file_data: Vec<u8>,
    pub file_format: String,
}

impl AdditionalFile {
    pub fn new(user_id: i32, file_name: String, file_data: Vec<u8>, file_format: String) -> Self {
        AdditionalFile {
            id: 0, // This will be set by the database
            user_id,
            file_name,
            file_data,
            file_format,
        }
    }

    pub async fn create(
        pool: &sqlx::PgPool,
        file: &AdditionalFile,
    ) -> Result<AdditionalFile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "INSERT INTO additional_files (user_id, file_name, file_data, file_format) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(&file.user_id)
        .bind(&file.file_name)
        .bind(&file.file_data)
        .bind(&file.file_format)
        .fetch_one(&mut *tx)
        .await;
        commit_else_rollback(tx, result).await
    }

    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<AdditionalFile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query_as::<_, AdditionalFile>("SELECT * FROM additional_files WHERE id = $1")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await
    }

    pub async fn delete(pool: &sqlx::PgPool, id: &i32) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query("DELETE FROM additional_files WHERE id = $1")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await;
        commit_else_rollback(tx, result).await?;
        Ok(())
    }

    pub async fn update(
        pool: &sqlx::PgPool,
        id: &i32,
        file: &AdditionalFile,
    ) -> Result<AdditionalFile, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let result = sqlx::query_as(
            "UPDATE additional_files SET user_id = $1, file_name = $2, file_data = $3, file_format = $4 WHERE id = $5 RETURNING *",
        )
        .bind(&file.user_id)
        .bind(&file.file_name)
        .bind(&file.file_data)
        .bind(&file.file_format)
        .bind(&id)
        .fetch_one(&mut *tx)
        .await;
        commit_else_rollback(tx, result).await
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
