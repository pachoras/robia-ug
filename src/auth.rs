use axum::{extract::FromRequestParts, http::header::AUTHORIZATION};

use crate::{
    api::{ApiError, ApiErrorTypes},
    models::{self, ApplicationToken, ProviderProfile, TokenTypeVariants, User},
    state,
};

use serde::Deserialize;

// Verify user's token
async fn verify_token(
    parts: &mut axum::http::request::Parts,
    state: &state::AppState,
) -> Result<ApplicationToken, ApiError> {
    if let Some(auth_header) = parts.headers.get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = auth_str.trim_start_matches("Bearer ").to_string();
                match models::ApplicationToken::find_by_token(&state.pool, &token).await {
                    Ok(matched_token) => match matched_token.verify().await {
                        Ok(verified_token) => {
                            return Ok(verified_token.clone());
                        }
                        Err(_) => {
                            return Err(ApiError {
                                message: "Invalid authentication token.".to_string(),
                                error_type: ApiErrorTypes::AuthenticationFailed,
                            });
                        }
                    },
                    Err(_) => {
                        return Err(ApiError {
                            message: "Invalid authentication token.".to_string(),
                            error_type: ApiErrorTypes::AuthenticationFailed,
                        });
                    }
                };
            } else {
                return Err(ApiError {
                    message: "Authorization header must start with Bearer: ".to_string(),
                    error_type: ApiErrorTypes::AuthenticationFailed,
                });
            }
        }
    }
    return Err(ApiError {
        message: "Missing or invalid authentication header".to_string(),
        error_type: ApiErrorTypes::AuthenticationFailed,
    });
}
pub struct ProvidesValidAuthentication(pub models::ApplicationToken);

impl FromRequestParts<state::AppState> for ProvidesValidAuthentication {
    type Rejection = ApiError;

    async fn from_request_parts(
        mut parts: &mut axum::http::request::Parts,
        state: &state::AppState,
    ) -> Result<Self, ApiError> {
        match verify_token(&mut parts, &state).await {
            Ok(token) => {
                if token.token_type == TokenTypeVariants::AdminAuthentication as i32
                    || token.token_type == TokenTypeVariants::ProAuthentication as i32
                    || token.token_type == TokenTypeVariants::LoansAuthentication as i32
                {
                    return Ok(ProvidesValidAuthentication(token));
                }
                return Err(ApiError {
                    message: "Invalid token".to_string(),
                    error_type: ApiErrorTypes::AuthenticationFailed,
                });
            }

            Err(_) => {
                return Err(ApiError {
                    message: "Missing or invalid authentication header".to_string(),
                    error_type: ApiErrorTypes::AuthenticationFailed,
                });
            }
        }
    }
}

/// After a valid token is available, get the user.
/// Note: Must be used *after ProvidesValidAuthentication
pub struct ProvidesUser(pub models::User);

impl FromRequestParts<state::AppState> for ProvidesUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &state::AppState,
    ) -> Result<Self, ApiError> {
        if let Some(auth_header) = parts.headers.get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str.trim_start_matches("Bearer ").to_string();
                    match User::find_by_token(&state.pool, &token).await {
                        Ok(user) => return Ok(ProvidesUser(user)),
                        Err(_) => {
                            return Err(ApiError {
                                message: "User doesn't exist".to_string(),
                                error_type: ApiErrorTypes::AuthenticationFailed,
                            });
                        }
                    }
                }
            }
        }
        return Err(ApiError {
            message: "Missing or invalid authentication header".to_string(),
            error_type: ApiErrorTypes::AuthenticationFailed,
        });
    }
}

/// After a valid token is available, get the provider.
/// Note: Must be used *after ProvidesValidAuthentication
pub struct ProvidesProvider(pub models::ProviderProfile);

impl FromRequestParts<state::AppState> for ProvidesProvider {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &state::AppState,
    ) -> Result<Self, ApiError> {
        if let Some(auth_header) = parts.headers.get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str.trim_start_matches("Bearer ").to_string();
                    match ProviderProfile::find_by_token(&state.pool, &token).await {
                        Ok(provider_profile) => return Ok(ProvidesProvider(provider_profile)),
                        Err(_) => {
                            return Err(ApiError {
                                message: "User doesn't exist".to_string(),
                                error_type: ApiErrorTypes::AuthenticationFailed,
                            });
                        }
                    }
                }
            }
        }
        return Err(ApiError {
            message: "Missing or invalid authentication header".to_string(),
            error_type: ApiErrorTypes::AuthenticationFailed,
        });
    }
}

/// Cannot be included with ProvidesAuthentication
pub struct ProvidesValidSubscription(pub models::ProviderProfile);

impl FromRequestParts<state::AppState> for ProvidesValidSubscription {
    type Rejection = ApiError;

    async fn from_request_parts(
        mut parts: &mut axum::http::request::Parts,
        state: &state::AppState,
    ) -> Result<Self, ApiError> {
        match verify_token(&mut parts, &state).await {
            Ok(token) => {
                match models::ProviderProfile::find_by_user_id(&state.pool, token.user_id).await {
                    Ok(profile) => {
                        // Check that provider has valid subscription
                        match models::Subscription::find_by_user_id(&state.pool, profile.user_id)
                            .await
                        {
                            Ok(subscription) => match subscription.validate().await {
                                Ok(_) => {
                                    return Ok(ProvidesValidSubscription(profile));
                                }
                                Err(_) => {
                                    return Err(ApiError {
                                        message: "Expired Subscription".to_string(),
                                        error_type: ApiErrorTypes::SubscriptionInvalid,
                                    });
                                }
                            },
                            Err(_) => {
                                return Err(ApiError {
                                    message: "No Subscription".to_string(),
                                    error_type: ApiErrorTypes::SubscriptionInvalid,
                                });
                            }
                        }
                    }
                    Err(_) => {
                        return Err(ApiError {
                            message: "Missing profile".to_string(),
                            error_type: ApiErrorTypes::UserNotFound,
                        });
                    }
                };
            }
            Err(_) => {
                return Err(ApiError {
                    message: "Missing or invalid authentication header".to_string(),
                    error_type: ApiErrorTypes::AuthenticationFailed,
                });
            }
        }
    }
}

// Methods to Verify google's id token

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String, // key ID
    n: String,   // RSA modulus (base64url)
    e: String,   // RSA exponent (base64url)
    alg: String,
    r#use: String,
}

async fn fetch_google_jwks() -> Result<Jwks, GoogleClaimsError> {
    let result = reqwest::get("https://www.googleapis.com/oauth2/v3/certs").await;
    if result.is_err() {
        return Err(GoogleClaimsError(format!(
            "Failed to fetch JWKS: {}",
            result
                .err()
                .ok_or_else(|| GoogleClaimsError("Failed to fetch data from Google".to_string()))?
        )));
    }
    let response = result.map_err(|e| GoogleClaimsError(format!("Failed to fetch JWKS: {}", e)))?;
    if !response.status().is_success() {
        return Err(GoogleClaimsError(format!(
            "Failed to fetch JWKS: HTTP {}",
            response.status()
        )));
    }
    let jwks_result: Result<Jwks, _> = response.json::<Jwks>().await;
    if jwks_result.is_err() {
        return Err(GoogleClaimsError(format!(
            "Failed to parse JWKS: {}",
            jwks_result
                .err()
                .ok_or_else(|| GoogleClaimsError("Unknown error".to_string()))?
        )));
    }
    let jwks =
        jwks_result.map_err(|e| GoogleClaimsError(format!("Failed to parse JWKS: {}", e)))?;
    Ok(jwks)
}

#[derive(Debug, Deserialize)]
pub struct GoogleClaims {
    pub sub: String, // unique user ID — use this as your user identifier
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub aud: String, // must match your client ID
    pub iss: String, // must be accounts.google.com
    pub exp: u64,
    pub iat: u64,
}

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

#[derive(Debug)]
pub struct GoogleClaimsError(String);

impl std::fmt::Display for GoogleClaimsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub async fn verify_google_token(token: &str) -> Result<GoogleClaims, GoogleClaimsError> {
    let google_client_id =
        std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set in .env");
    // 1. Decode header to get `kid` (which key was used to sign)
    let header = decode_header(token)
        .map_err(|e| GoogleClaimsError(format!("Failed to decode JWT header: {}", e)))?;
    let kid = header
        .kid
        .ok_or_else(|| GoogleClaimsError("Missing kid in JWT header".to_string()))?;

    // 2. Fetch Google's public keys and find the matching one
    let jwks = fetch_google_jwks().await?;
    let jwk: &Jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| GoogleClaimsError(format!("No matching key found for kid: {}", kid)))?;

    // 3. Build the decoding key from RSA n + e
    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| GoogleClaimsError(format!("Failed to create decoding key: {}", e)))?;

    // 4. Set up validation rules
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[google_client_id]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);

    // 5. Decode + verify (signature, exp, aud, iss all checked here)
    let token_data = decode::<GoogleClaims>(token, &decoding_key, &validation)
        .map_err(|e| GoogleClaimsError(format!("Token verification failed: {}", e)))?;
    let claims = token_data.claims;

    // 6. Extra safety check
    if !claims.email_verified {
        return Err(GoogleClaimsError("Google email not verified".to_string()));
    }

    Ok(claims)
}
