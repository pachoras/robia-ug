use axum::{
    extract::FromRequestParts,
    http::{
        StatusCode,
        header::{AUTHORIZATION, HeaderValue},
    },
};

use crate::state;
use crate::{models, responses::AppError};

use serde::Deserialize;

pub struct ExtractAuthenticationCode(pub HeaderValue);

impl FromRequestParts<state::AppState> for ExtractAuthenticationCode {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &state::AppState,
    ) -> Result<Self, AppError> {
        if let Some(auth_header) = parts.headers.get(AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str.trim_start_matches("Bearer ").to_string();
                    let database_token =
                        models::UserAuthToken::find_by_token(&state.pool, &token).await;
                    if database_token.is_err() {
                        return Err(AppError {
                            status_code: StatusCode::UNAUTHORIZED,
                            message: Some("Invalid authentication token".to_string()),
                        });
                    }
                    let code = ExtractAuthenticationCode(
                        HeaderValue::from_str(
                            &database_token
                                .map_err(|_| AppError {
                                    status_code: StatusCode::UNAUTHORIZED,
                                    message: Some("Invalid authentication token".to_string()),
                                })?
                                .token,
                        )
                        .map_err(|_| AppError {
                            status_code: StatusCode::UNAUTHORIZED,
                            message: Some("Invalid authentication token".to_string()),
                        })?,
                    );
                    return Ok(code);
                } else {
                    return Err(AppError {
                        status_code: StatusCode::UNAUTHORIZED,
                        message: Some("Authorization header must start with 'Bearer '".to_string()),
                    });
                }
            }
        }
        return Err(AppError {
            status_code: StatusCode::UNAUTHORIZED,
            message: Some("Missing or invalid Authorization header".to_string()),
        });
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
