use crate::models;
use axum::extract::Multipart;

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
        let file_name = field
            .file_name()
            .ok_or(FormError("Missing file name".to_string()))?
            .to_string();
        let data = field.bytes().await.map_err(|e| FormError(e.to_string()))?;

        if name == "email" {
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
            let file_extension = file_name.split('.').last().unwrap_or("").to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "proof_of_address_error".to_string(),
                    "Proof of address must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.proof_of_address_file_format = file_extension;
                profile_data.proof_of_address = data.to_vec();
            }
        }
        if name == "national_id_front" {
            // Check for supported file types (PDF, JPEG, PNG)
            let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
            let file_extension = file_name.split('.').last().unwrap_or("").to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "national_id_front_error".to_string(),
                    "National ID front must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.national_id_front_file_format = file_extension;
                profile_data.national_id_front = data.to_vec();
            }
        }
        if name == "national_id_back" {
            // Check for supported file types (PDF, JPEG, PNG)
            let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
            let file_extension = file_name.split('.').last().unwrap_or("").to_lowercase();
            if !allowed_extensions.contains(&file_extension.as_str()) {
                context.insert(
                    "national_id_back_error".to_string(),
                    "National ID back must be a PDF, JPEG, or PNG file".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            } else {
                profile_data.national_id_back_file_format = file_extension;
                profile_data.national_id_back = data.to_vec();
            }
        }
    }
    if context.contains_key("errors") {
        return Err(FormError(
            "Please correct the errors in the form".to_string(),
        ));
    }
    Ok((user_data, profile_data))
}
