use std::collections::HashMap;

use axum::extract::Multipart;
use regex;
use serde::{Deserialize, Serialize};

fn verify_phone_number(phone_number: &str) -> bool {
    let re = regex::Regex::new(r"^\+?256[1-9]\d{8}$").unwrap();
    re.is_match(phone_number)
}

fn verify_email(email: &str) -> bool {
    let re = regex::Regex::new(r"^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,4}$").unwrap();
    re.is_match(email)
}

fn verify_file_extension(file_name: &Option<String>) -> Result<String, FormError> {
    let allowed_extensions = ["pdf", "jpg", "jpeg", "png"];
    let file_extension = &file_name
        .clone()
        .unwrap_or_else(|| "".to_string())
        .split('.')
        .last()
        .unwrap_or("")
        .to_lowercase();
    if !allowed_extensions.contains(&file_extension.as_str()) {
        return Err(FormError("Invalid file extension".to_string()));
    }
    Ok(file_extension.to_string())
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

#[derive(Debug)]
pub struct FormError(String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
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
            if !verify_email(&String::from_utf8_lossy(&data)) {
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
            if !verify_phone_number(&String::from_utf8_lossy(&data)) {
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
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.proof_of_address_file_format = file_extension;
                    profile_data.proof_of_address = data.to_vec();
                }
                Err(_) => {
                    context.insert(
                        "proof_of_address_error".to_string(),
                        "Proof of address must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
            }
        }
        if name == "national_id_front" {
            // Check for supported file types (PDF, JPEG, PNG)
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.national_id_front_file_format = file_extension;
                    profile_data.national_id_front = data.to_vec();
                }
                Err(_) => {
                    context.insert(
                        "national_id_front_error".to_string(),
                        "National ID front must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
            }
        }
        if name == "national_id_back" {
            // Check for supported file types (PDF, JPEG, PNG)
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.national_id_back_file_format = file_extension;
                    profile_data.national_id_back = data.to_vec();
                }
                Err(_) => {
                    context.insert(
                        "national_id_back_error".to_string(),
                        "National ID back must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
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

#[derive(Clone, Debug, Deserialize)]
pub struct ProviderProfileData {
    pub user_id: i32,
    pub business_name: String,
    pub employee_name: String,
    pub employee_national_id: String,
    pub phone_number: String,
    pub employee_count: i32,
    pub certificate_of_incorporation: String,
    pub certificate_of_incorporation_file_format: String,
    pub loan_license: String,
    pub loan_license_file_format: String,
    pub business_proof_of_address: String,
    pub business_proof_of_address_file_format: String,
}

impl ProviderProfileData {
    pub fn new() -> Self {
        ProviderProfileData {
            user_id: 0,
            business_name: String::new(),
            employee_name: String::new(),
            employee_national_id: String::new(),
            phone_number: String::new(),
            employee_count: 0,
            certificate_of_incorporation: String::new(),
            certificate_of_incorporation_file_format: String::new(),
            loan_license: String::new(),
            loan_license_file_format: String::new(),
            business_proof_of_address: String::new(),
            business_proof_of_address_file_format: String::new(),
        }
    }
}

pub async fn get_provider_registration_form_data(
    mut multipart: Multipart,
) -> Result<(UserData, ProviderProfileData), FormError> {
    let mut user_data = UserData::new();
    let mut profile_data = ProviderProfileData::new();
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
        let file_name = field.file_name().map(|s| s.to_string());
        let data = field.bytes().await.map_err(|e| FormError(e.to_string()))?;

        if name == "email" {
            if !verify_email(&String::from_utf8_lossy(&data)) {
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
        if name == "business_name" {
            profile_data.business_name =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if profile_data.business_name.is_empty() {
                context.insert(
                    "business_name_error".to_string(),
                    "Business name is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "employee_name" {
            profile_data.employee_name =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if profile_data.employee_name.is_empty() {
                context.insert(
                    "employee_name_error".to_string(),
                    "Employee name is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "employee_national_id" {
            profile_data.employee_national_id =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            if profile_data.employee_national_id.is_empty() {
                context.insert(
                    "employee_national_id_error".to_string(),
                    "Employee national ID is required".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "phone_number" {
            if !verify_phone_number(&String::from_utf8_lossy(&data)) {
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
        if name == "employee_count" {
            let count_str =
                String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
            profile_data.employee_count = count_str
                .parse::<i32>()
                .map_err(|e| FormError(e.to_string()))?;
            if profile_data.employee_count <= 0 {
                context.insert(
                    "employee_count_error".to_string(),
                    "Employee count must be a positive integer".to_string(),
                );
                context.insert("errors".to_string(), "true".to_string());
            }
        }
        if name == "certificate_of_incorporation" {
            // Check for supported file types (PDF, JPEG, PNG)
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.certificate_of_incorporation_file_format = file_extension;
                    profile_data.certificate_of_incorporation =
                        String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
                }
                Err(_) => {
                    context.insert(
                        "certificate_of_incorporation_error".to_string(),
                        "Certificate of incorporation must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
            }
        }
        if name == "loan_license" {
            // Check for supported file types (PDF, JPEG, PNG)
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.loan_license_file_format = file_extension;
                    profile_data.loan_license =
                        String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
                }
                Err(_) => {
                    context.insert(
                        "loan_license_error".to_string(),
                        "Loan license must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
            }
        }
        if name == "business_proof_of_address" {
            // Check for supported file types (PDF, JPEG, PNG)
            match verify_file_extension(&file_name) {
                Ok(file_extension) => {
                    profile_data.business_proof_of_address_file_format = file_extension;
                    profile_data.business_proof_of_address =
                        String::from_utf8(data.to_vec()).map_err(|e| FormError(e.to_string()))?;
                }
                Err(_) => {
                    context.insert(
                        "business_proof_of_address_error".to_string(),
                        "Business proof of address must be a PDF, JPEG, or PNG file".to_string(),
                    );
                    context.insert("errors".to_string(), "true".to_string());
                }
            }
        }
    }
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
