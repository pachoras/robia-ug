use lettre::{
    Message, SmtpTransport, Transport,
    message::header::ContentType,
    transport::smtp::authentication::{Credentials, Mechanism},
};

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
    let mut tera = tera::Tera::new("templates/**/*").unwrap();
    tera.add_template_file("src/templates/email.html", Some("email.html"))
        .unwrap();
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
    let sender = SmtpTransport::relay(&smtp_host)?
        // Add credentials for authentication
        .credentials(Credentials::new(smtp_username, smtp_password))
        // Optionally configure expected authentication mechanism
        .authentication(vec![Mechanism::Plain])
        .build();

    sender.send(&email)?;
    Ok(())
}
