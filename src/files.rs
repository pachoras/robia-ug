use axum::body::Bytes;

pub fn upload_file(name: &str, data: &Bytes) {
    // Handle file upload (e.g., save to cloud storage)
    log::info!("File uploaded successfully: {}", name);
}
