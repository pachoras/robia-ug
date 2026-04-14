use std::collections::HashMap;

use axum::extract::Multipart;
use regex;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct FormError(String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct UserData {
    pub email: String,
}

impl UserData {
    pub fn new() -> Self {
        UserData {
            email: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserProfileData {
    pub user_id: i32,
    pub full_name: String,
    pub phone_number: String,
    pub proof_of_address: Vec<u8>,
    pub proof_of_address_file_format: String,
    pub national_id_back: Vec<u8>,
    pub national_id_back_file_format: String,
    pub national_id_front: Vec<u8>,
    pub national_id_front_file_format: String,
    pub google_id: Option<String>,
    pub additional_files: Option<HashMap<String, Vec<u8>>>,
}

impl UserProfileData {
    pub fn new() -> Self {
        UserProfileData {
            user_id: 0,
            full_name: String::new(),
            phone_number: String::new(),
            proof_of_address: Vec::new(),
            proof_of_address_file_format: String::new(),
            national_id_back: Vec::new(),
            national_id_back_file_format: String::new(),
            national_id_front: Vec::new(),
            national_id_front_file_format: String::new(),
            google_id: None,
            additional_files: None,
        }
    }
}

/// Helper function to extract and validate registration form data from the multipart request.
pub async fn get_seeker_registration_form_data(
    mut multipart: Multipart,
) -> Result<(UserData, UserProfileData), FormError> {
    let mut user_data = UserData::new();
    let mut profile_data = UserProfileData::new();
    let mut context = std::collections::HashMap::new();
    let mut additional_file_map: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| FormError(e.to_string()))?
    {
        // Get form fields
        let name = field
            .name()
            .ok_or(FormError("Missing field name".to_string()))?
            .to_string();
        let file_name = field.file_name().map(|s| s.to_string());
        let data = field.bytes().await.map_err(|e| FormError(e.to_string()))?;

        if name == "email" {
            let re = regex::Regex::new(r"^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,4}$").unwrap();
            if !re.is_match(&String::from_utf8_lossy(&data)) {
                context.insert(
                    "email_error".to_string(),
                    "Invalid email format".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
            user_data.email =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if user_data.email.is_empty() {
                context.insert("email_error".to_string(), "Email is required".to_string());
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "full_name" {
            profile_data.full_name =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if profile_data.full_name.is_empty() {
                context.insert(
                    "full_name_error".to_string(),
                    "Full name is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "phone_number" {
            let re = regex::Regex::new(r"^\+?256[1-9]\d{8}$").unwrap();
            if !re.is_match(&String::from_utf8_lossy(&data)) {
                context.insert(
                    "phone_number_error".to_string(),
                    "Invalid phone number format".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
            profile_data.phone_number =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if profile_data.phone_number.is_empty() {
                context.insert(
                    "phone_number_error".to_string(),
                    "Phone number is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "proof_of_address" {
            // Check for supported file types (PDF, JPEG, PNG)
            let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
            let file_extension = &file_name
                .clone()
                .ok_or(FormError("Missing file name".to_string()))?
                .split('.')
                .last()
                .unwrap_or("")
                .to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "proof_of_address_error".to_string(),
                    "Proof of address must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.proof_of_address_file_format = file_extension.to_string();
                profile_data.proof_of_address = data.to_vec();
            }
        }
        if name == "national_id_front" {
            // Check for supported file types (PDF, JPEG, PNG)
            let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
            let file_extension = &file_name
                .clone()
                .ok_or(FormError("Missing file name".to_string()))?
                .split('.')
                .last()
                .unwrap_or("")
                .to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "national_id_front_error".to_string(),
                    "National ID front must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.national_id_front_file_format = file_extension.to_string();
                profile_data.national_id_front = data.to_vec();
            }
        }
        if name == "national_id_back" {
            // Check for supported file types (PDF, JPEG, PNG)
            let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
            let file_extension = &file_name
                .clone()
                .ok_or(FormError("Missing file name".to_string()))?
                .split('.')
                .last()
                .unwrap_or("")
                .to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "national_id_back_error".to_string(),
                    "National ID back must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.national_id_back_file_format = file_extension.to_string();
                profile_data.national_id_back = data.to_vec();
            }
        } else if name.contains("additional_file") {
            // Store additional files in a vector
            let filename = &file_name
                .clone()
                .ok_or(FormError("Missing file name".to_string()))?;
            additional_file_map.insert(filename.clone(), data.to_vec());
        }
    }
    profile_data.additional_files = Some(additional_file_map);
    if context.contains_key("errors") {
        let error_messages: Vec<String> = context
            .iter()
            .filter(|(key, _)| key.ends_with("_error"))
            .map(|(_, value)| value.clone())
            .collect();
        return Err(FormError(format!(
            "Please correct the errors in the form: {}",
            error_messages.join(", ")
        )));
    }
    Ok((user_data, profile_data))
}

pub async fn validate_password(password: &str) -> Result<(), FormError> {
    if password.len() < 8 {
        return Err(FormError(
            "Password must be at least 8 characters long".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err(FormError(
            "Password must contain at least one uppercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        return Err(FormError(
            "Password must contain at least one lowercase letter".to_string(),
        ));
    }
    if !password.chars().any(|c| c.is_digit(10)) {
        return Err(FormError(
            "Password must contain at least one digit".to_string(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgotPasswordData {
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginData {
    pub email: String,
    pub password: String,
    pub application: Option<String>,
}
