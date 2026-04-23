//! This module describes functions that make end to end transactions, e.g user registration.
//!
//! # Usage
//!
//! ```
//! use my_crate::utils;
//! let result = utils::process(vec![1, 2, 3]);
//! ```
//!
//! For more details, see [`process`](utils::process).

use reqwest::StatusCode;

use crate::{
    forms::{self, ProviderProfileData, UserData, UserProfileData},
    mail::{send_password_reset_email, send_welcome_email},
    models::{self, ApplicationToken, User},
    responses::{AppError, ErrorPopupResponse, SuccessPopupResponse},
    utils,
};
/// Creates a new registration token for a user. Returns the created token or an error if there's a database issue.
pub async fn create_registration_token(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &User,
    user_type: models::TokenTypeVariants,
) -> Result<ApplicationToken, sqlx::Error> {
    let create_token = ApplicationToken::new(user.id, user_type, utils::generate_random_string(64));
    match ApplicationToken::create(tx, &create_token).await {
        Ok(token) => {
            // Send verification email
            send_welcome_email(user.clone(), token.token.clone()).await;
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
    // Prepare transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: Some(format!(
                "Could not check for existing user at this time: {}",
                e
            )),
        })
        .unwrap();
    let create_token = ApplicationToken::new(
        user.id,
        models::TokenTypeVariants::PasswordReset,
        utils::generate_random_string(64),
    );
    // First delete any existing password reset tokens for the user to prevent multiple valid tokens at the same time
    match ApplicationToken::find_any_by_user_id_and_type(
        pool,
        user.id,
        models::TokenTypeVariants::PasswordReset,
    )
    .await
    {
        Ok(existing_tokens) => {
            for existing_token in existing_tokens {
                match ApplicationToken::delete(&mut tx, &existing_token.token).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!(
                            "Error deleting existing authentication token for user ID {}: {}",
                            user.id,
                            e
                        );
                        return Err(e);
                    }
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

    match ApplicationToken::create(&mut tx, &create_token).await {
        Ok(token) => {
            // Send email
            send_password_reset_email(user.email.clone(), token.token.clone()).await;
            Ok(token)
        }
        Err(e) => return Err(e),
    }
}
/// Creates a new authentication token for a user based on the application variant. Returns the created token or an error if there's a database issue.
pub async fn create_auth_token(
    pool: &sqlx::PgPool,
    user_id: i32,
    token_variant: models::TokenTypeVariants,
) -> Result<ApplicationToken, sqlx::Error> {
    // Prepare transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: Some(format!(
                "Could not check for existing user at this time: {}",
                e
            )),
        })
        .unwrap();
    // First delete any existing tokens for this user to prevent multiple valid tokens at the same time
    match ApplicationToken::find_any_by_user_id_and_type(pool, user_id, token_variant.clone()).await
    {
        Ok(existing_tokens) => {
            for existing_token in existing_tokens {
                match ApplicationToken::delete(&mut tx, &existing_token.token).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!(
                            "Error deleting existing authentication token for user ID {}: {}",
                            user_id,
                            e
                        );
                        return Err(e);
                    }
                }
            }
        }
        Err(sqlx::Error::RowNotFound) => {
            // No existing token found, continue with creating new one
        }
        Err(e) => {
            log::error!(
                "Error checking for existing authentication token for user ID {}: {}",
                user_id,
                e
            );
            return Err(e);
        }
    }

    // Then create new registration token
    let create_token =
        ApplicationToken::new(user_id, token_variant, utils::generate_random_string(64));

    match ApplicationToken::create(&mut tx, &create_token).await {
        Ok(token) => {
            tx.commit().await?;
            Ok(token)
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}
// Update a user's data
pub async fn update_password<'a>(
    pool: &sqlx::PgPool,
    tera: &'a mut tera::Tera,
    token: &String,
    new_password: &String,
) -> Result<SuccessPopupResponse<'a>, ErrorPopupResponse<'a>> {
    let mut err: Option<String> = None;
    match models::ApplicationToken::find_by_token(pool, token).await {
        Ok(token) => {
            // Validate token
            match token.verify().await {
                Ok(app_token) => {
                    // Validate new password
                    match forms::validate_password(new_password).await {
                        Ok(_) => {
                            // Mark token as used
                            match models::ApplicationToken::set_used(&pool, &app_token.id).await {
                                Ok(_) => {
                                    // Create new password hash
                                    match models::User::find(&pool, app_token.user_id).await {
                                        Ok(mut user) => {
                                            user.password_hash = crate::utils::get_password_hash(
                                                new_password,
                                                &user.salt,
                                            );
                                            match pool.begin().await {
                                                Ok(mut tx) => {
                                                    match models::User::update(
                                                        &mut tx, user.id, &user,
                                                    )
                                                    .await
                                                    {
                                                        Ok(_) => {
                                                            log::info!(
                                                                "Password updated for user ID: {}",
                                                                user.id
                                                            );
                                                            // Redirect to login page with success message
                                                            tx.commit()
                                                                .await
                                                                .map_err(|_| AppError {
                                                                    status_code:
                                                                        StatusCode::INTERNAL_SERVER_ERROR,
                                                                    message: Some(
                                                                        "Unable to create user".to_string(),
                                                                    ),
                                                                })
                                                                .unwrap();
                                                        }
                                                        Err(e) => {
                                                            log::error!(
                                                                "Error updating password: {}",
                                                                e
                                                            );
                                                            tx.rollback()
                                                                .await
                                                                .map_err(|_| AppError {
                                                                    status_code:
                                                                        StatusCode::INTERNAL_SERVER_ERROR,
                                                                    message: Some(
                                                                        "Unable to create user".to_string(),
                                                                    ),
                                                                })
                                                                .unwrap();
                                                            err = Some(
                                                                "Unable to create user".to_string(),
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("Database pool Error, {}", e);
                                                    err = Some("Unable to create user".to_string());
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            err = Some("User doesn't exist".to_string());
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Cannot verify token: {}", e);
                                    err = Some(format!("Password validation error: {}", e));
                                }
                            };
                        }
                        Err(e) => {
                            log::error!("Password validation error: {}", e);
                            err = Some(format!("Password validation error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error verifying registration token: {}", e);
                    err = Some("Error verifying registration token".to_string());
                }
            }
        }
        Err(e) => {
            log::error!("Invalid registration token: {}", e);
            err = Some("Invalid registration token".to_string());
        }
    };
    match err {
        Some(_) => {
            return Err(ErrorPopupResponse {
                message: err.unwrap().to_string(),
                tera: tera,
                path: "src/templates/change_password.html",
                context: {
                    let mut c = std::collections::HashMap::new();
                    c.insert("token".to_string(), token.clone());
                    c
                },
            });
        }
        None => {
            return Ok(SuccessPopupResponse {
                message: "Your password has been updated successfully. Please log in.".to_string(),
                tera: tera,
                path: "src/templates/login.html",
                context: std::collections::HashMap::new(),
            });
        }
    }
}
/// Creates a new loan applicant user and sends them a verification email. Returns the created user or an error if there's a database issue.
pub async fn create_application_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    email: &String,
    token_type: models::TokenTypeVariants,
) -> Result<User, sqlx::Error> {
    let user = User::new(email.clone(), utils::generate_random_string(12));
    match User::create(tx, &user).await {
        Ok(created_user) => {
            // Also create registration token for user
            create_registration_token(tx, &created_user, token_type).await?;
            log::info!("Created user with ID: {}", created_user.id);
            Ok(created_user)
        }
        Err(e) => return Err(e),
    }
}
/// Create a new user from uncommitted form data
pub async fn register_user<'a>(
    pool: &sqlx::PgPool,
    client: aws_sdk_s3::Client,
    tera: &'a mut tera::Tera,
    user_data: &UserData,
    profile_data: &UserProfileData,
) -> Result<SuccessPopupResponse<'a>, AppError> {
    let mut err: Option<AppError> = None;
    // Prepare transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: Some(format!(
                "Could not check for existing user at this time: {}",
                e
            )),
        })
        .unwrap();

    // Check if user with phone number or national ID already exists
    match models::UserProfile::find_by_phone_number(&pool, &profile_data.phone_number).await {
        Ok(_) => {
            return Err(AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some(
                    "A user with this profile already exists. Please log in instead.".to_string(),
                ),
            });
        }
        Err(sqlx::Error::RowNotFound) => {
            // No user with this phone number, continue
        }
        Err(e) => {
            return Err(AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: Some(format!(
                    "Could not check for existing user at this time: {}",
                    e
                )),
            });
        }
    }

    // Create user
    match create_application_user(
        &mut tx,
        &user_data.email,
        models::TokenTypeVariants::LoansAuthentication,
    )
    .await
    {
        Ok(user) => {
            // Create user profile
            let mut updated_profile_data = profile_data.clone();
            updated_profile_data.user_id = user.id;
            match models::UserProfile::create(&mut tx, &client, &updated_profile_data).await {
                Ok(profile) => {
                    log::info!("Created user profile for user ID: {}", profile.user_id);
                    // Continue
                }
                Err(e) => {
                    log::error!("Error creating user profile: {}", e);
                    err = Some(AppError {
                        status_code: StatusCode::INTERNAL_SERVER_ERROR,
                        message: Some("Could not create user profile at this time.".to_string()),
                    });
                }
            }
        }
        Err(sqlx::Error::Database(e)) => {
            if e.code() == Some("23505".into()) {
                err = Some(AppError {
                    status_code: StatusCode::BAD_REQUEST,
                    message: Some("A user with this email already exists.".to_string()),
                });
            } else {
                log::error!("Error creating user: {}", e);
                err = Some(AppError {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                    message: Some("Could not create user at this time.".to_string()),
                });
            }
        }
        Err(e) => {
            log::error!("Error creating user: {}", e);
            err = Some(AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: Some("Could not create user at this time.".to_string()),
            });
        }
    };
    match err {
        Some(_) => match tx.rollback().await {
            Ok(_) => {}
            Err(e) => {
                return Err(AppError {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                    message: Some(format!(
                        "Could not check for existing user at this time: {}",
                        e
                    )),
                });
            }
        },
        None => match tx.commit().await {
            Ok(_) => {}
            Err(e) => {
                return Err(AppError {
                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                    message: Some(format!(
                        "Could not check for existing user at this time: {}",
                        e
                    )),
                });
            }
        },
    };
    Ok(SuccessPopupResponse {
        message: "Your loan application has been received, please check your email for further instructions.".to_string(),
        tera: tera,
        path: "src/templates/index.html",
        context: std::collections::HashMap::new(),
    })
}
/// Create a new provider from uncommitted form data
pub async fn register_provider<'a>(
    pool: &sqlx::PgPool,
    client: aws_sdk_s3::Client,
    tera: &'a mut tera::Tera,
    user_data: &UserData,
    profile_data: &ProviderProfileData,
) -> Result<SuccessPopupResponse<'a>, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: None,
        })
        .unwrap();
    let mut err: Option<AppError> = None;
    let mut user: Option<User> = None;
    // Check if provider already exists
    match models::User::find_by_email(&pool, &user_data.email).await {
        Ok(found_user) => {
            // Check if the existing profile is already verified
            match models::ProviderProfile::find_by_user_id(pool, found_user.id).await {
                Ok(existing_profile) => {
                    if !existing_profile.is_verified {
                        // Assume resubmission and delete old profile and associated files before creating new one
                        match models::ProviderProfile::delete(&mut tx, existing_profile.id).await {
                            Ok(_) => {
                                log::info!(
                                    "Deleted unverified provider profile with ID: {}",
                                    existing_profile.id
                                );
                            }
                            Err(e) => {
                                log::error!("Error deleting existing provider profile: {}", e);
                                return Err(AppError {
                                    status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                    message: Some(
                                        "Could not process your application at this time."
                                            .to_string(),
                                    ),
                                });
                            }
                        }
                    } else {
                        // Return if verified already
                        return Err(AppError {
                            status_code: StatusCode::BAD_REQUEST,
                            message: Some("A provider with this email already exists.".to_string()),
                        });
                    }
                }
                Err(_) => {
                    // Continue, a new profile will be created
                    user = Some(found_user);
                }
            }
        }
        Err(sqlx::Error::RowNotFound) => {
            // Continue, a new user will be created
        }
        Err(_) => {
            log::error!("Error looking up usesr: {}", user_data.email);
            return Err(AppError {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: Some("Could not check for existing provider at this time.".to_string()),
            });
        }
    };
    // Check that submitted data isn't already assigned to another profile
    match models::ProviderProfile::verify_unique_provider_profile(
        &pool,
        &profile_data.business_name,
        &profile_data.phone_number,
    )
    .await
    {
        Ok(_) => {
            // Create a new user if one doesn't exist
            if user.is_none() {
                match create_application_user(
                    &mut tx,
                    &user_data.email,
                    models::TokenTypeVariants::ProEmailVerification,
                )
                .await
                {
                    Ok(new_user) => {
                        user = Some(new_user);
                    }
                    Err(e) => {
                        log::error!("Error creating new user provider: {}", e);
                        err = Some(AppError {
                            status_code: StatusCode::INTERNAL_SERVER_ERROR,
                            message: Some("Could not create user at this time.".to_string()),
                        });
                    }
                };
            };
            // Create a new profile
            let mut new_profile_data = profile_data.clone();
            // We can unwrap, as user is not none
            new_profile_data.user_id = user.unwrap().id;
            match models::ProviderProfile::create(&mut tx, &new_profile_data, &client).await {
                Ok(new_profile) => {
                    log::info!(
                        "Created provider profile for user ID: {}",
                        new_profile.user_id
                    );
                    // Commit if no errors, or rollback
                    if err.is_none() {
                        tx.commit()
                            .await
                            .map_err(|_| AppError {
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                message: None,
                            })
                            .unwrap();
                        return Ok(SuccessPopupResponse {
                            message: "Your provider application has been received, please check your email for further instructions.".to_string(),
                            tera: tera,
                            path: "src/templates/index.html",
                            context: std::collections::HashMap::new(),
                        });
                    } else {
                        tx.rollback()
                            .await
                            .map_err(|_| AppError {
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                                message: None,
                            })
                            .unwrap();
                        return Err(err.unwrap());
                    }
                }
                Err(e) => {
                    log::error!("Error creating provider profile: {}", e);
                    return Err(AppError {
                        status_code: StatusCode::INTERNAL_SERVER_ERROR,
                        message: Some(
                            "Could not create provider profile at this time.".to_string(),
                        ),
                    });
                }
            }
        }
        Err(_) => {
            // Provider is not unique, return an error
            return Err(AppError {
                status_code: StatusCode::BAD_REQUEST,
                message: Some("A provider with this profile already exists.".to_string()),
            });
        }
    };
}
