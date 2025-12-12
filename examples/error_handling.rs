//! Example: Proper error handling with retries.
//!
//! This example demonstrates how to handle errors properly, including
//! implementing retry logic based on error retryability.
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! cargo run --example error_handling
//! ```

use email_sync::matcher::OtpMatcher;
use email_sync::{Error, ImapConfig, ImapEmailClient};
use std::env;
use std::time::Duration;

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Connect with automatic retry for transient failures
async fn connect_with_retry(config: &ImapConfig) -> Result<ImapEmailClient, Error> {
    let mut last_error = None;
    let mut backoff = INITIAL_BACKOFF;

    for attempt in 1..=MAX_RETRIES {
        println!("Connection attempt {}/{}...", attempt, MAX_RETRIES);

        match ImapEmailClient::connect(config.clone()).await {
            Ok(client) => {
                println!("Connected successfully!");
                return Ok(client);
            }
            Err(e) => {
                println!("  Error: {}", e);
                println!("  Category: {}", e.category());
                println!("  Retryable: {}", e.is_retryable());

                if e.is_retryable() && attempt < MAX_RETRIES {
                    println!("  Retrying in {:?}...", backoff);
                    tokio::time::sleep(backoff).await;
                    backoff *= 2; // Exponential backoff
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap())
}

/// Search with error classification
async fn search_with_error_handling(client: &mut ImapEmailClient) -> Result<Option<String>, Error> {
    let matcher = OtpMatcher::six_digit();
    let max_age = Duration::from_secs(3600); // Last hour

    match client.find_recent_match(&matcher, max_age).await {
        Ok(code) => Ok(Some(code)),
        Err(e) => {
            // Handle different error categories
            match e.category() {
                email_sync::ErrorCategory::NotFound => {
                    // No match is expected - not an error condition
                    println!("No matching email found (this is normal)");
                    Ok(None)
                }
                email_sync::ErrorCategory::Network | email_sync::ErrorCategory::Timeout => {
                    // Network issues might be transient
                    println!("Network/timeout error (retryable): {}", e);
                    Err(e)
                }
                email_sync::ErrorCategory::Protocol => {
                    // IMAP protocol errors
                    println!("Protocol error: {}", e);
                    Err(e)
                }
                email_sync::ErrorCategory::Parse => {
                    // Email parsing failed
                    println!("Parse error (likely malformed email): {}", e);
                    // Could continue to next email in real implementation
                    Ok(None)
                }
                email_sync::ErrorCategory::Configuration => {
                    // Configuration errors shouldn't happen here
                    println!("Configuration error: {}", e);
                    Err(e)
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let email = env::var("EMAIL_ADDRESS").expect("EMAIL_ADDRESS environment variable required");
    let password =
        env::var("EMAIL_PASSWORD").expect("EMAIL_PASSWORD environment variable required");

    println!("Email Sync - Error Handling Example\n");
    println!("====================================\n");

    // Build configuration
    let config = match ImapConfig::builder()
        .email(&email)
        .password(password)
        .connect_timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            eprintln!("This error is NOT retryable - fix your configuration");
            std::process::exit(1);
        }
    };

    // Connect with retry logic
    let mut client = match connect_with_retry(&config).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\nFailed to connect after {} attempts", MAX_RETRIES);
            eprintln!("Final error: {}", e);
            std::process::exit(1);
        }
    };

    // Search with proper error handling
    println!("\nSearching for OTP codes...");
    match search_with_error_handling(&mut client).await {
        Ok(Some(code)) => println!("Found OTP: {}", code),
        Ok(None) => println!("No OTP found in recent emails"),
        Err(e) => eprintln!("Search failed: {}", e),
    }

    // Always try to logout, but don't fail if it doesn't work
    println!("\nLogging out...");
    if let Err(e) = client.logout().await {
        eprintln!("Logout error (non-critical): {}", e);
    }

    println!("Done!");
}
