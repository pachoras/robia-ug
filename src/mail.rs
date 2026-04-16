use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
    transport::smtp::authentication::{Credentials, Mechanism},
};

use crate::renderer::init_renderer;

/// Sends an email using lettre
pub async fn send_email(
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
    link: &str,
    action: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Render email from template
    let mut tera = init_renderer();
    tera.add_template_file("src/templates/email.html", Some("email.html"))
        .unwrap_or_else(|e| panic!("Failed to add template file: {}", e));
    let mut context = tera::Context::new();
    context.insert("title", subject);
    context.insert("message", body);
    context.insert("link", link);
    context.insert("action", action);
    let rendered = tera.render("email.html", &context)?;
    let email = Message::builder()
        .from(from.parse()?)
        .reply_to(from.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(rendered)?;

    // Create the SMTPS transport
    let smtp_host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.example.com".to_string());
    let smtp_username = std::env::var("SMTP_USERNAME").unwrap_or_else(|_| "username".to_string());
    let smtp_password = std::env::var("SMTP_PASSWORD").unwrap_or_else(|_| "password".to_string());
    let sender = AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)?
        .credentials(Credentials::new(smtp_username, smtp_password))
        .authentication(vec![Mechanism::Plain])
        .build();

    sender.send(email).await?;
    Ok(())
}

/// Sends an email to admins when a 500 error occurs in production
pub fn send_admin_error_email(error_message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let debug_mode = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string());
    if debug_mode.contains("debug") {
        log::info!("Debug mode enabled, skipping sending admin error email.");
        return Ok(());
    }
    let _error_message = error_message.to_string();
    // Send email in the background without blocking the main thread
    tokio::spawn(async move {
        let admin_email =
            std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
        let from_email =
            std::env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@example.com".to_string());
        send_email(
            &from_email,
            &admin_email,
            "500 Error Occurred in Application",
            &format!(
                "A 500 error occurred in the application:\n\n{}",
                _error_message.to_string()
            ),
            "",
            "View Logs",
        )
        .await
        .map_err(|e| {
            log::error!("Failed to send admin error email: {}", e);
            e
        })
        .unwrap();
    });
    Ok(())
}
