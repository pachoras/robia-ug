use std::collections::HashMap;

use axum::{
    extract::FromRequestParts,
    http::{
        StatusCode,
        header::{AUTHORIZATION, HeaderValue},
    },
};

use crate::state;
use crate::{models, routes::AppError};

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
                        return Err(AppError(
                            StatusCode::UNAUTHORIZED,
                            Some("Invalid authentication token".to_string()),
                        ));
                    }
                    let code = ExtractAuthenticationCode(
                        HeaderValue::from_str(&database_token.unwrap().token).unwrap(),
                    );
                    return Ok(code);
                } else {
                    return Err(AppError(
                        StatusCode::UNAUTHORIZED,
                        Some("Authorization header must start with 'Bearer '".to_string()),
                    ));
                }
            }
        }
        return Err(AppError(
            StatusCode::UNAUTHORIZED,
            Some("Missing or invalid Authorization header".to_string()),
        ));
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

// {
//   "keys": [
//     {
//       "alg": "RS256",
//       "kty": "RSA",
//       "kid": "a10e58df55e728566ec56bda6eb3bd45439f35d7",
//       "n": "xlXYB0tN6zPS5ab4yjmCTnmYGwIigMDY0YqW3hYYrYOdXbZX9XWqKVO-XpqKgWY9EBGe15AWRUq2-uEASiSXZef8wdMjSBwUpSKdYSiAZCvjaO39c6nhdlZ57kGNd_oULOrFHWoLmO-7LP368E8H5BhmgjQzLhvl2BTdSX5IaTwxMBxZzhts2Ql-RkoNtm30_p9Wz-rWe9_mHotXLFB6zHjziH2VN3HJcBVcJFb2NCp4oQUFtCJd4u_6y3WvFfMtvPo6c7hthFhaDnEV_SIHtAViBtjtP-JETnLCUNCXAoMWJwCDzHllyavc6IbUWgNNCqRRvkBDDF9IxJBomDHqrQ",
//       "e": "AQAB",
//       "use": "sig"
//     },
//     {
//       "kid": "cce4e024a51aa0c1c41c1a4515a41dd7e961936b",
//       "alg": "RS256",
//       "kty": "RSA",
//       "e": "AQAB",
//       "use": "sig",
//       "n": "t2siQKIKIwl-kCuxm5hL3IjoBdhHyZ0cjZr46q30LOMFc-9jCEsU7JkkoKLH8C0xjtwVS8i36ksVK1sjpib6SchY40nZG2prZbLdJji0IfCD6lYP_xEgobq2IdRt3X8Vf4k4OUhwckcFy1cod4139jFGnMzcVmE8LXujOigeAYQMAXop0wpkVFudzhMqhTH3rhHjt12ZJ-e1HRKvc7EAD0NoG_FaxWrsUJtl44FnaHxoRU_pRIuALrxCXvgEjcLDivbXwTGXldNW1R3ilsWi2Q1ERx6vXg4UDMg-9YtFKWekjt29xvEp3phchl2SV82rDkb7k16JC1zwsGpDLP55SQ"
//     }
//   ]
// }

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
