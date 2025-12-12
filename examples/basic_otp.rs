//! Basic example: Wait for an OTP code from email.
//!
//! This example demonstrates the most common use case - connecting to an IMAP
//! server and waiting for an email containing a 6-digit OTP code.
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! cargo run --example basic_otp
//! ```
//!
//! For Gmail, you'll need to use an [App Password](https://support.google.com/accounts/answer/185833).

use email_sync::matcher::OtpMatcher;
use email_sync::{ImapConfig, ImapEmailClient};
use std::env;

#[tokio::main]
async fn main() -> email_sync::Result<()> {
    // Read credentials from environment
    let email = env::var("EMAIL_ADDRESS").expect("EMAIL_ADDRESS environment variable required");
    let password =
        env::var("EMAIL_PASSWORD").expect("EMAIL_PASSWORD environment variable required");

    println!("Connecting to IMAP server for {}...", email);

    // Build configuration - IMAP host is auto-discovered from email domain
    let config = ImapConfig::builder()
        .email(&email)
        .password(password)
        .build()?;

    // Connect to IMAP server
    let mut client = ImapEmailClient::connect(config).await?;

    println!("Connected! Waiting for OTP code...");
    println!("(Send yourself an email with a 6-digit code, or press Ctrl+C to cancel)");

    // Wait for an email containing a 6-digit OTP code
    let otp = client.wait_for_match(&OtpMatcher::six_digit()).await?;

    println!("Got OTP code: {}", otp);

    // Clean up
    client.logout().await?;

    Ok(())
}
