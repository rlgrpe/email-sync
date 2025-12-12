//! Example: Using tracing for observability.
//!
//! This example demonstrates how to enable structured logging using
//! the `tracing` ecosystem. All major operations in email-sync emit
//! tracing spans and events.
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! # Set log level (trace, debug, info, warn, error)
//! export RUST_LOG=email_sync=debug
//!
//! cargo run --example with_tracing
//! ```

use email_sync::matcher::OtpMatcher;
use email_sync::{ImapConfig, ImapEmailClient};
use std::env;
use std::time::Duration;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> email_sync::Result<()> {
    // Initialize tracing subscriber with environment filter
    // Use RUST_LOG environment variable to control log levels
    // Example: RUST_LOG=email_sync=debug,info
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("email_sync=info")),
        )
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    let email = env::var("EMAIL_ADDRESS").expect("EMAIL_ADDRESS environment variable required");
    let password =
        env::var("EMAIL_PASSWORD").expect("EMAIL_PASSWORD environment variable required");

    tracing::info!(email = %email, "Starting email-sync example");

    let config = ImapConfig::builder()
        .email(&email)
        .password(password)
        .poll_interval(Duration::from_secs(5))
        .max_wait(Duration::from_secs(30))
        .build()?;

    tracing::debug!("Configuration built successfully");

    // Connect - this will emit spans for connection, TLS, and authentication
    let mut client = ImapEmailClient::connect(config).await?;

    tracing::info!("Connection established, searching for OTP codes");

    // Search recent emails - this will emit spans for search and fetch operations
    match client
        .find_recent_match(&OtpMatcher::six_digit(), Duration::from_secs(3600))
        .await
    {
        Ok(code) => {
            tracing::info!(otp = %code, "Found OTP code");
            println!("\nFound OTP: {}", code);
        }
        Err(e) => {
            tracing::warn!(error = %e, "No OTP code found");
            println!("\nNo OTP found: {}", e);
        }
    }

    // Logout - emits a span for the logout operation
    client.logout().await?;

    tracing::info!("Example completed successfully");

    Ok(())
}
