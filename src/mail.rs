use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::{Credentials, Mechanism},
};

use crate::{models::User, renderer::init_renderer};

/// Sends an email using lettre
pub async fn send_email(
    to: &str,
    subject: &str,
    template: &str,
    mut context: tera::Context,
) -> Result<(), Box<dyn std::error::Error>> {
    // Render email from template
    let mut tera = init_renderer();
    tera.add_template_file(format!("src/templates/{}", template), Some(template))
        .unwrap_or_else(|e| panic!("Failed to add template file: {}", e));
    context.insert("title", subject);
    let rendered = tera.render(template, &context)?;
    let no_reply: Mailbox = "Robia Labs <no-reply@robialabs.com>".parse()?;
    let email = Message::builder()
        .from(no_reply.clone())
        .reply_to(no_reply)
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
    match sender.send(email).await {
        Ok(_) => {}
        Err(e) => {
            log::error!("Failed to send email: {}", e.to_string())
        }
    };
    Ok(())
}

/// Sends an email to admins when a 500 error occurs in production
pub fn send_admin_error_email(error_message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let debug_mode = std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".to_string());
    if debug_mode.contains("debug") {
        log::info!("Debug mode enabled, skipping sending admin error email.");
        return Ok(());
    }
    let admin_email =
        std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@example.com".to_string());
    let body = format!(
        "A 500 error occurred in the application:\n\n{}",
        error_message.to_string()
    );
    // Send email in the background without blocking the main thread
    tokio::spawn(async move {
        let mut context = tera::Context::new();
        context.insert("message", &body);
        send_email(
            &admin_email,
            "500 Error Occurred in Application",
            "email.html",
            context,
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

/// Send welcome email from html template
pub async fn send_welcome_email(user: User, token: String) {
    // Send verification email
    let hostname =
        std::env::var("HOSTNAME").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let link = format!("{}/verify-token/{}", hostname, token);
    let body = format!(
        r#"Your loan application has been received, please click the link below to complete your
        registration and view your loan application status.

        If you cannot click the link, please copy and paste the following URL into your browser:    {}"#,
        link
    );
    // Send email in background task to avoid blocking the main thread
    tokio::spawn(async move {
        let mut context = tera::Context::new();
        context.insert("message", &body);
        context.insert("link", &link);
        context.insert("action", "Verify Email");
        send_email(&user.email, "Welcome To Robia", "email.html", context)
            .await
            .map_err(|e| {
                log::error!("Failed to send admin error email: {}", e);
                e
            })
            .unwrap();
    });
}

/// Send password reset email from html template
pub async fn send_password_reset_email(user_email: String, token: String) {
    // Send verification email
    let hostname =
        std::env::var("HOSTNAME").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let link = format!("{}/verify-token/{}", hostname, token);
    let body = format!(
        r#"You recently requested a password reset. Please click the link below to reset your password.

        If you did not request a password reset, please ignore this email or reply to let us know.
        This password reset link is only valid for the next 24 hours.

        If you cannot click the link, please copy and
        paste the following URL into your browser:    {}"#,
        link
    );
    // Send email in background task to avoid blocking the main thread
    tokio::spawn(async move {
        let mut context = tera::Context::new();
        context.insert("message", &body);
        context.insert("link", &link);
        context.insert("action", "Verify Email");
        send_email(&user_email, "Welcome To Robia", "email.html", context)
            .await
            .map_err(|e| {
                log::error!("Failed to send admin error email: {}", e);
                e
            })
            .unwrap();
    });
}

/// Send new subscription email from html template
pub async fn send_new_subscription_email(
    user_email: String,
    amount: f64,
    total: f64,
    subscription: String,
) {
    // Send verification email
    let hostname = std::env::var("PRO_APPLICATION_URL")
        .unwrap_or_else(|_| "http://localhost:4000".to_string());
    // Send email in background task to avoid blocking the main thread
    tokio::spawn(async move {
        let mut context = tera::Context::new();
        context.insert("amount", &amount);
        context.insert("total", &total);
        context.insert("link", &hostname);
        context.insert("subscription", &subscription);
        send_email(
            &user_email,
            "Subscription Active",
            "subscription_email.html",
            context,
        )
        .await
        .map_err(|e| {
            log::error!("Failed to send admin error email: {}", e);
            e
        })
        .unwrap();
    });
}
