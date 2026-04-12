use std::hash::Hash;

use crate::models;
use aws_sdk_s3::types::error;
use axum::extract::Multipart;
use regex;

#[derive(Debug)]
pub struct FormError(String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Helper function to extract and validate registration form data from the multipart request.
pub async fn get_seeker_registration_form_data(
    mut multipart: Multipart,
) -> Result<(models::UserData, models::UserProfileData), FormError> {
    let mut user_data = models::UserData::new();
    let mut profile_data = models::UserProfileData::new();
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
