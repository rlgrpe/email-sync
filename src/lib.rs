//! # email-sync
//!
//! Async IMAP email client for monitoring mailboxes and extracting content using pattern matching.
//!
//! This crate provides a high-level, async API for:
//! - Connecting to IMAP servers (with optional SOCKS5 proxy support)
//! - Waiting for emails matching specific patterns (OTP codes, activation links, etc.)
//! - Finding recent emails matching patterns
//!
//! ## Features
//!
//! - **`observability`**: Enables OpenTelemetry integration for distributed tracing.
//!   Without this feature, tracing spans are still emitted but require no OTEL dependencies.
//!
//! ## Quick Start
//!
//! ```no_run
//! use email_sync::{ImapConfig, ImapEmailClient};
//! use email_sync::matcher::OtpMatcher;
//!
//! # async fn example() -> email_sync::Result<()> {
//! // Configure the client
//! let config = ImapConfig::builder()
//!     .email("user@gmail.com")
//!     .password("app-password")  // Use app-specific password for Gmail
//!     .build()?;
//!
//! // Connect to IMAP server
//! let mut client = ImapEmailClient::connect(config).await?;
//!
//! // Wait for a 6-digit OTP code
//! let otp = client.wait_for_match(&OtpMatcher::six_digit()).await?;
//! println!("Got OTP: {}", otp);
//!
//! // Clean up
//! client.logout().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Using a SOCKS5 Proxy
//!
//! ```no_run
//! use email_sync::{ImapConfig, ImapEmailClient, Socks5Proxy};
//! use email_sync::matcher::OtpMatcher;
//!
//! # async fn example() -> email_sync::Result<()> {
//! let config = ImapConfig::builder()
//!     .email("user@gmail.com")
//!     .password("app-password")
//!     .proxy(Socks5Proxy::with_auth("proxy.example.com", 1080, "user", "pass"))
//!     .build()?;
//!
//! let mut client = ImapEmailClient::connect(config).await?;
//! // ... use client ...
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Pattern Matching
//!
//! ```
//! use email_sync::matcher::{RegexMatcher, Matcher};
//!
//! // Extract a token from email
//! let matcher = RegexMatcher::new(r"token=([a-f0-9]{32})").unwrap();
//!
//! // Or use a closure for complex logic
//! use email_sync::matcher::ClosureMatcher;
//! use std::borrow::Cow;
//!
//! let custom = ClosureMatcher::new(
//!     |text| {
//!         // Custom extraction logic
//!         text.lines()
//!             .find(|line| line.starts_with("SECRET:"))
//!             .map(|line| Cow::Owned(line.trim_start_matches("SECRET:").trim().to_string()))
//!     },
//!     "secret extractor"
//! );
//! ```
//!
//! ## RAII Guard for Automatic Cleanup
//!
//! ```no_run
//! use email_sync::{ImapConfig, ImapEmailClient};
//! use email_sync::matcher::OtpMatcher;
//!
//! # async fn example() -> email_sync::Result<()> {
//! # let config = ImapConfig::builder().email("a@b.c").password("x").build()?;
//! let client = ImapEmailClient::connect(config).await?;
//! let mut guard = client.into_guard();  // Will logout on drop
//!
//! let code = guard.wait_for_match(&OtpMatcher::six_digit()).await?;
//! // Guard automatically logs out when dropped
//! # Ok(())
//! # }
//! ```
//!
//! ## Error Handling
//!
//! All errors implement `std::error::Error` and provide context. Use [`Error::is_retryable`]
//! to determine if an operation can be retried:
//!
//! ```
//! use email_sync::Error;
//!
//! fn handle_error(error: &Error) {
//!     if error.is_retryable() {
//!         println!("Transient error, can retry: {}", error);
//!     } else {
//!         println!("Permanent error: {}", error);
//!     }
//! }
//! ```
//!
//! ## Observability
//!
//! The crate uses `tracing` for instrumentation. All major operations emit spans with
//! structured fields suitable for distributed tracing.
//!
//! ### Span Naming Convention
//!
//! - `ImapEmailClient::connect` - Client connection
//! - `ImapEmailClient::wait_for_match` - Waiting for email
//! - `ImapEmailClient::find_recent_match` - Finding recent email
//! - `ImapEmailClient::logout` - Logout
//! - `session::authenticate` - IMAP authentication
//! - `connection::establish_tls` - TLS connection
//!
//! ### Standard Fields
//!
//! - `email` - Email address (masked in production)
//! - `imap_host` - IMAP server hostname
//! - `proxy_enabled` - Whether proxy is used
//! - `matcher` - Matcher description
//! - `uid` - Email UID
//!
//! Enable the `observability` feature for OpenTelemetry integration.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// Public modules
pub mod config;
pub mod error;
pub mod known_servers;
pub mod matcher;
pub mod proxy;

// Internal modules
mod client;
mod connection;
mod parser;
mod session;

// Re-exports for ergonomic API
pub use client::{ImapEmailClient, ImapEmailClientGuard};
pub use config::{ImapConfig, ImapConfigBuilder, PollingConfig, TimeoutConfig};
pub use email_address::EmailAddress;
pub use error::{Error, ErrorCategory, Result};
pub use known_servers::ServerRegistry;
pub use proxy::{ProxyAuth, Socks5Proxy};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_api_accessible() {
        // Ensure all public types are accessible
        let _ = ImapConfig::builder();
        let _ = Socks5Proxy::new("localhost", 1080);
        let _ = matcher::OtpMatcher::six_digit();
    }
}
