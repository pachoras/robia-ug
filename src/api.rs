use axum::{Json, extract::State, response::IntoResponse};
use serde_json::{Value, json};

use crate::{auth::verify_google_token, models, state::AppState, utils};

pub async fn login_google(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Decode JWT and get aud claim
    match payload.get("token").and_then(|v| v.as_str()) {
        Some(token) => {
            match verify_google_token(token).await {
                Ok(claims) => {
                    // Check if user with email exists in database
                    match models::User::find_by_email(&state.pool, &claims.email).await {
                        Ok(profile) => {
                            log::info!(
                                "User with email {} found, ID: {}",
                                claims.email,
                                profile.user_id
                            );
                            // If user exists, create auth token and return it in response
                            let app = payload
                                .get("app")
                                .and_then(|v| v.as_str())
                                .unwrap_or("loans");
                            let user_auth_token = models::UserAuthToken::new(
                                profile.user_id,
                                app.to_string(),
                                utils::generate_random_string(32),
                            );
                            match models::UserAuthToken::create(&state.pool, &user_auth_token).await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    log::error!("Error creating user auth token: {}", e);
                                    return Json(
                                        json!({"status": "ERROR", "error": "Could not create auth token at this time."}),
                                    );
                                }
                            };
                            // Also save user google id in database for future reference
                            let mut update_profile = profile.clone();
                            update_profile.google_id = Some(claims.sub.clone());
                            match models::UserProfile::update(
                                &state.pool,
                                update_profile.id,
                                &update_profile,
                            )
                            .await
                            {
                                Ok(_) => {
                                    return Json(
                                        json!({"status": "OK", "auth_token": &user_auth_token.token}),
                                    );
                                }
                                Err(e) => {
                                    log::error!(
                                        "Error updating Google ID for user with email {}: {}",
                                        claims.email,
                                        e
                                    );
                                    return Json(
                                        json!({"status": "ERROR", "error": "Could not update user profile with Google ID."}),
                                    );
                                }
                            }
                        }
                        Err(sqlx::Error::RowNotFound) => {
                            return Json(
                                json!({"status": "NOT_FOUND", "error": "No user with this email found. Please sign up first."}),
                            );
                        }
                        Err(e) => {
                            log::error!("Database error checking for user by email: {}", e);
                            return Json(
                                json!({"status": "ERROR", "error": "Could not check for user at this time."}),
                            );
                        }
                    }
                }
                Err(err) => {
                    log::error!("Error decoding JWT: {}", err);
                    return Json(json!({"status": "ERROR", "error": "Invalid token"}));
                }
            }
        }
        None => {
            log::error!("Missing token in request payload");
            return Json(json!({"status": "MISSING", "error": "Missing token"}));
        }
    }
}
