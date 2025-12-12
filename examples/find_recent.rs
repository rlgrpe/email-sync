//! Example: Find a matching email from recent messages.
//!
//! Unlike `wait_for_match`, this example shows how to search through
//! existing recent emails without polling for new ones.
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! cargo run --example find_recent
//! ```

use email_sync::matcher::{OtpMatcher, RegexMatcher, UrlMatcher};
use email_sync::{ImapConfig, ImapEmailClient};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> email_sync::Result<()> {
    let email = env::var("EMAIL_ADDRESS").expect("EMAIL_ADDRESS environment variable required");
    let password =
        env::var("EMAIL_PASSWORD").expect("EMAIL_PASSWORD environment variable required");

    println!("Connecting to IMAP server for {}...", email);

    let config = ImapConfig::builder()
        .email(&email)
        .password(password)
        .build()?;

    let mut client = ImapEmailClient::connect(config).await?;

    println!("Connected! Searching recent emails...\n");

    // Search for different patterns in emails from the last 24 hours
    let max_age = Duration::from_secs(24 * 3600);

    // Try to find a 6-digit OTP code
    println!("Looking for 6-digit OTP codes...");
    match client
        .find_recent_match(&OtpMatcher::six_digit(), max_age)
        .await
    {
        Ok(code) => println!("  Found OTP: {}", code),
        Err(e) => println!("  No OTP found: {}", e),
    }

    // Try to find a GitHub URL
    println!("\nLooking for GitHub URLs...");
    match client
        .find_recent_match(&UrlMatcher::new("github.com"), max_age)
        .await
    {
        Ok(url) => println!("  Found URL: {}", url),
        Err(e) => println!("  No GitHub URL found: {}", e),
    }

    // Try to find a verification token
    println!("\nLooking for verification tokens...");
    let token_matcher =
        RegexMatcher::new(r"token[=:]?\s*([a-zA-Z0-9]{16,})").expect("valid regex pattern");
    match client.find_recent_match(&token_matcher, max_age).await {
        Ok(token) => println!("  Found token: {}", token),
        Err(e) => println!("  No token found: {}", e),
    }

    client.logout().await?;

    println!("\nDone!");
    Ok(())
}
