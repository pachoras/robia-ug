use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::postgres::PgRow;
use sqlx::types::chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::forms::ProviderProfileData;
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
    pub google_id: Option<String>,
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
            google_id: Some("id".to_string()),
            is_enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Creates a new user. Returns the created user or an error if there's a database issue.
    /// Note that this function does not commit (flush) contents
    pub async fn create<'a>(
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        create_user: &User,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (email, password_hash, salt) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_user.email)
        .bind(&create_user.password_hash)
        .bind(&create_user.salt)
        .fetch_one(&mut **tx)
        .await
    }
    /// Deletes a user by its ID. Returns an error if there's a database issue.
    pub async fn delete<'a>(
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        id: i32,
    ) -> Result<Option<PgRow>, sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&id)
            .fetch_optional(&mut **tx)
            .await
    }
    /// Reads a user by its ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: i32) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(&id)
            .fetch_one(pool)
            .await
    }
    /// Finds a user profile by its email. Returns an error if not found or if there's a database issue.
    pub async fn find_by_email(pool: &sqlx::PgPool, email: &String) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(pool)
            .await
    }
    /// Updates a user by its ID. Returns the updated user or an error if there's a database issue.
    pub async fn update<'a>(
        tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
        id: i32,
        data: &User,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
                "UPDATE users SET email = $1, password_hash = $2, salt = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $4 RETURNING *",
            )
            .bind(&data.email)
            .bind(&data.password_hash)
            .bind(&data.salt)
            .bind(&id)
            .fetch_one(&mut **tx)
            .await
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
            created_at: Utc::now(),
            updated_at: Utc::now(), // This will be set by the database
        }
    }
    /// Creates a new user profile. Returns the created profile or an error if there's a database issue.
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        s3_client: &aws_sdk_s3::Client,
        profile: &forms::UserProfileData,
    ) -> Result<UserProfile, sqlx::Error> {
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

        let result: Result<UserProfile, sqlx::Error> = sqlx::query_as(
            r#"
            INSERT INTO user_profiles (user_id, full_name, phone_number, proof_of_address,
                national_id_front, national_id_back, google_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"#,
        )
        .bind(&profile.user_id)
        .bind(&profile.full_name)
        .bind(&profile.phone_number)
        .bind(&proof_of_address_file_name)
        .bind(&national_id_front_file_name)
        .bind(&national_id_back_file_name)
        .bind(&profile.google_id)
        .fetch_one(&mut **tx)
        .await;

        let _client = s3_client.clone();
        let _profile = profile.clone();

        // Prepare data map with file paths to upload
        let mut data_map = HashMap::new();
        if let Some(additional_files) = &_profile.additional_files {
            for (file_name, data) in additional_files.iter() {
                let file_path = files::get_additional_file_path(&_profile.user_id, file_name);
                let _path = file_path.clone();
                let data = Bytes::from(data.clone());
                data_map.insert(file_path, data);
                // Also create additional file entries in database
                let file = AdditionalFile::new(_profile.user_id, &_path);
                AdditionalFile::create(tx, &file).await?;
            }
        }

        // Upload additional files in the background
        tokio::spawn(async move {
            // Set up data objects for upload
            let proof_of_address_data = Bytes::from(_profile.proof_of_address.clone());
            data_map.insert(proof_of_address_file_name.clone(), proof_of_address_data);

            let national_id_front_data = Bytes::from(_profile.national_id_front.clone());
            data_map.insert(national_id_front_file_name.clone(), national_id_front_data);

            let national_id_back_data = Bytes::from(_profile.national_id_back.clone());
            data_map.insert(national_id_back_file_name.clone(), national_id_back_data);

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
        result
    }
    /// Reads a user profile by its user ID. Returns an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, user_id: i32) -> Result<UserProfile, sqlx::Error> {
        sqlx::query_as("SELECT * FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(pool)
            .await
    }
    /// Updates a user profile by its user ID. Returns the updated profile or an error if there's a database issue.
    pub async fn update(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        profile: &UserProfile,
    ) -> Result<UserProfile, sqlx::Error> {
        sqlx::query_as("UPDATE user_profiles SET full_name = $1, phone_number = $2, proof_of_address = $3, national_id_front = $4, national_id_back = $5 WHERE user_id = $6 RETURNING *")
            .bind(&profile.full_name)
            .bind(&profile.phone_number)
            .bind(&profile.proof_of_address)
            .bind(&profile.national_id_front)
            .bind(&profile.national_id_back)
            .bind(&profile.user_id)
            .fetch_one(&mut **tx)
            .await
    }
    /// Deletes a user profile by its user ID. Returns an error if there's a database issue.
    pub async fn delete(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM user_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_optional(&mut **tx)
            .await
            .unwrap();
        Ok(())
    }
    /// Finds a user profile by its phone number. Returns an error if not found or if there's a database issue.
    pub async fn find_by_phone_number(
        pool: &sqlx::PgPool,
        phone_number: &String,
    ) -> Result<UserProfile, sqlx::Error> {
        sqlx::query_as("SELECT * FROM user_profiles WHERE phone_number = $1")
            .bind(&phone_number)
            .fetch_one(pool)
            .await
    }
    /// Finds a user profile by its email. Returns an error if not found or if there's a database issue.
    pub async fn find_by_email(
        pool: &sqlx::PgPool,
        email: &String,
    ) -> Result<UserProfile, sqlx::Error> {
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE email = $1")
            .bind(&email)
            .fetch_one(pool)
            .await
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: i32,
    pub user_id: i32,
    pub business_name: String,
    pub employee_name: String,
    pub employee_national_id: String,
    pub phone_number: String,
    pub employee_count: i32,
    pub certificate_of_incorporation: String,
    pub loan_license: String,
    pub business_proof_of_address: String,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderProfile {
    // Create new empty provider profile object
    pub fn new(
        user_id: i32,
        business_name: String,
        employee_name: String,
        employee_national_id: String,
        phone_number: String,
        employee_count: i32,
        certificate_of_incorporation: String,
        loan_license: String,
        business_proof_of_address: String,
    ) -> Self {
        ProviderProfile {
            id: 0, // This will be set by the database
            user_id,
            business_name,
            employee_name,
            employee_national_id,
            phone_number,
            employee_count,
            certificate_of_incorporation,
            loan_license,
            business_proof_of_address,
            is_verified: false,
            created_at: Utc::now(),
            updated_at: Utc::now(), // This will be set by the database
        }
    }
    /// Creates a new provider profile. Returns the created profile or an error if there's a database issue.
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        profile: &ProviderProfileData,
        client: &aws_sdk_s3::Client,
    ) -> Result<ProviderProfile, sqlx::Error> {
        let business_proof_of_address_path_file_name = files::get_business_proof_of_address_path(
            &profile.user_id,
            &profile.business_proof_of_address_file_format,
        );
        let business_loan_license_path_file_name = files::get_business_loan_license_path(
            &profile.user_id,
            &profile.loan_license_file_format,
        );
        let certificate_of_incorporation_file_name = files::get_certificate_of_incorporation_path(
            &profile.user_id,
            &profile.certificate_of_incorporation_file_format,
        );

        log::info!("{}", business_proof_of_address_path_file_name);
        log::info!("{}", business_loan_license_path_file_name);
        log::info!("{}", certificate_of_incorporation_file_name);

        let result = sqlx::query_as(r#"
            INSERT INTO provider_profiles (user_id, business_name, employee_name, employee_national_id, phone_number,
            employee_count, certificate_of_incorporation, loan_license, business_proof_of_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *"#)
            .bind(&profile.user_id)
            .bind(&profile.business_name)
            .bind(&profile.employee_name)
            .bind(&profile.employee_national_id)
            .bind(&profile.phone_number)
            .bind(&profile.employee_count)
            .bind(&certificate_of_incorporation_file_name)
            .bind(&business_loan_license_path_file_name)
            .bind(&business_proof_of_address_path_file_name)
            .fetch_one(&mut **tx).await;
        // Also upload files
        let _profile = profile.clone();
        let _client = client.clone();
        tokio::spawn(async move {
            // Set up data objects for upload
            let mut data_map = HashMap::new();
            let proof_of_address_data = Bytes::from(_profile.business_proof_of_address.clone());
            data_map.insert(
                business_proof_of_address_path_file_name.clone(),
                proof_of_address_data,
            );
            let loan_license_data = Bytes::from(_profile.loan_license.clone());
            data_map.insert(
                business_loan_license_path_file_name.clone(),
                loan_license_data,
            );
            let certificate_of_incorporation_data =
                Bytes::from(_profile.certificate_of_incorporation.clone());
            data_map.insert(
                certificate_of_incorporation_file_name.clone(),
                certificate_of_incorporation_data,
            );
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
        result
    }
    /// Finds a provider profile by ID. Returns the profile or an error if not found or if there's a database issue.
    pub async fn find(pool: &sqlx::PgPool, id: i32) -> Result<ProviderProfile, sqlx::Error> {
        sqlx::query_as("SELECT * FROM provider_profiles WHERE id = $1")
            .bind(&id)
            .fetch_one(pool)
            .await
    }
    /// Finds a provider profile by user ID.
    pub async fn find_by_user_id(
        pool: &sqlx::PgPool,
        user_id: i32,
    ) -> Result<ProviderProfile, sqlx::Error> {
        sqlx::query_as("SELECT * FROM provider_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(pool)
            .await
    }
    /// Find provider profile by business name
    pub async fn find_by_business_name(
        pool: &sqlx::PgPool,
        business_name: &String,
    ) -> Result<ProviderProfile, sqlx::Error> {
        sqlx::query_as::<_, ProviderProfile>(
            "SELECT * FROM provider_profiles WHERE business_name = $1",
        )
        .bind(&business_name)
        .fetch_one(pool)
        .await
    }
    /// Find provider profile by phone number
    pub async fn find_by_phone_number(
        pool: &sqlx::PgPool,
        phone_number: &String,
    ) -> Result<ProviderProfile, sqlx::Error> {
        sqlx::query_as::<_, ProviderProfile>(
            "SELECT * FROM provider_profiles WHERE phone_number = $1",
        )
        .bind(&phone_number)
        .fetch_one(pool)
        .await
    }
    /// Deletes a provider profile by user ID. Returns an error if there's a database issue.
    pub async fn delete(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM provider_profiles WHERE user_id = $1")
            .bind(&user_id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(())
    }
    /// Verifies that no other profile has the same email, business name or phone number.
    /// Returns the profile or an error if not found or if there's a database issue.
    pub async fn verify_unique_provider_profile(
        pool: &sqlx::PgPool,
        business_name: &String,
        phone_number: &String,
    ) -> Result<(), sqlx::Error> {
        // Check business name uniqueness
        match ProviderProfile::find_by_business_name(&pool, &business_name).await {
            Ok(_) => {
                log::error!("Profile found, business name not unique: {}", business_name);
                return Err(sqlx::Error::InvalidArgument(
                    "Business name must be unique".to_string(),
                ));
            }
            Err(_) => {
                // Continue
            }
        };
        //Check phone number uniqueness
        match ProviderProfile::find_by_phone_number(&pool, &phone_number).await {
            Ok(_) => {
                log::error!(
                    "Profile found, business phone_number not unique: {}",
                    phone_number
                );
                return Err(sqlx::Error::RowNotFound);
            }
            Err(_) => {
                // Continue
            }
        };
        Ok(())
    }
}

pub enum SubscriptionTypeVariants {
    Beginner,
    Pro,
    Ultimate,
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct Subscription {
    pub id: i32,
    pub user_id: i32,
    pub subscription_type: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
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
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        create_token: &ApplicationToken,
    ) -> Result<ApplicationToken, sqlx::Error> {
        sqlx::query_as::<_, ApplicationToken>(
            "INSERT INTO application_tokens (token, user_id, token_type) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(&create_token.token)
        .bind(&create_token.user_id)
        .bind(&create_token.token_type)
        .fetch_one(&mut **tx)
        .await
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
        sqlx::query_as::<_, ApplicationToken>("SELECT * FROM application_tokens WHERE token = $1")
            .bind(&token)
            .fetch_one(pool)
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
    /// Verifies a token by its token string.
    pub async fn verify(self, pool: &sqlx::PgPool) -> Result<ApplicationToken, sqlx::Error> {
        let app_token = ApplicationToken::find_by_token(pool, &self.token).await?;
        // Check if token has been used
        if app_token.is_used {
            log::error!("Token {} has already been used", self.token);
            return Err(sqlx::Error::InvalidArgument(
                "Token already used".to_string(),
            ));
        }
        // Check if token is expired (valid for 24 hours)
        let now = Utc::now();
        if now.signed_duration_since(app_token.created_at).num_hours() >= 24 {
            log::error!("Token {} has expired", self.token);
            return Err(sqlx::Error::InvalidArgument(
                "Token {} has expired".to_string(),
            ));
        }
        Ok(app_token)
    }
}

#[derive(Clone, Debug, FromRow, Serialize, Deserialize)]
pub struct AdditionalFile {
    pub id: i32,
    pub user_id: i32,
    pub file_name: String,
}

impl AdditionalFile {
    pub fn new(user_id: i32, file_name: &String) -> Self {
        AdditionalFile {
            id: 0,
            user_id,
            file_name: file_name.to_string(),
        }
    }
    /// Save additional file details to database
    pub async fn create(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        file: &AdditionalFile,
    ) -> Result<AdditionalFile, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO additional_files (user_id, file_name) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(&file.user_id)
        .bind(&file.file_name)
        .fetch_one(&mut **tx)
        .await
    }
    /// Find additional file by id
    pub async fn find(pool: &sqlx::PgPool, id: &i32) -> Result<AdditionalFile, sqlx::Error> {
        sqlx::query_as::<_, AdditionalFile>("SELECT * FROM additional_files WHERE id = $1")
            .bind(&id)
            .fetch_one(pool)
            .await
    }
    /// Delete additional file
    pub async fn delete(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: &i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM additional_files WHERE id = $1")
            .bind(&id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(())
    }
    /// Update the additional file
    pub async fn update(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: &i32,
        file: &AdditionalFile,
    ) -> Result<AdditionalFile, sqlx::Error> {
        sqlx::query_as(
            "UPDATE additional_files SET user_id = $1, file_name = $2 WHERE id = $5 RETURNING *",
        )
        .bind(&file.user_id)
        .bind(&file.file_name)
        .bind(&id)
        .fetch_one(&mut **tx)
        .await
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
