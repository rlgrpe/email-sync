//! Integration tests for email-sync.
//!
//! These tests require a real IMAP server and are disabled by default.
//! To run them:
//!
//! ```bash
//! # Set environment variables
//! export EMAIL_SYNC_TEST_EMAIL="your@email.com"
//! export EMAIL_SYNC_TEST_PASSWORD="your-app-password"
//!
//! # Optional: proxy configuration
//! export EMAIL_SYNC_TEST_PROXY_HOST="proxy.example.com"
//! export EMAIL_SYNC_TEST_PROXY_PORT="1080"
//!
//! # Run with the integration-tests feature
//! cargo test --features integration-tests -- --ignored
//! ```

use email_sync::matcher::{ClosureMatcher, OtpMatcher, RegexMatcher, UrlMatcher};
use email_sync::{ImapConfig, ImapEmailClient, Socks5Proxy};
use std::borrow::Cow;
use std::env;
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Test Configuration Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn get_test_credentials() -> Option<(String, String)> {
    dotenvy::dotenv().ok();
    let email = env::var("EMAIL_SYNC_TEST_EMAIL").ok()?;
    let password = env::var("EMAIL_SYNC_TEST_PASSWORD").ok()?;
    Some((email, password))
}

fn get_test_proxy() -> Option<Socks5Proxy> {
    let host = env::var("EMAIL_SYNC_TEST_PROXY_HOST").ok()?;
    let port: u16 = env::var("EMAIL_SYNC_TEST_PROXY_PORT").ok()?.parse().ok()?;

    let proxy = match (
        env::var("EMAIL_SYNC_TEST_PROXY_USER").ok(),
        env::var("EMAIL_SYNC_TEST_PROXY_PASS").ok(),
    ) {
        (Some(user), Some(pass)) => Socks5Proxy::with_auth(&host, port, user, pass),
        _ => Socks5Proxy::new(host, port),
    };

    Some(proxy)
}

fn get_test_config() -> Option<ImapConfig> {
    let (email, password) = get_test_credentials()?;

    let mut builder = ImapConfig::builder().email(email).password(password);

    if let Some(proxy) = get_test_proxy() {
        builder = builder.proxy(proxy);
    }

    builder.build().ok()
}

fn get_test_config_with_short_timeout() -> Option<ImapConfig> {
    let (email, password) = get_test_credentials()?;

    let mut builder = ImapConfig::builder()
        .email(email)
        .password(password)
        .max_wait(Duration::from_secs(5))
        .poll_interval(Duration::from_secs(1));

    if let Some(proxy) = get_test_proxy() {
        builder = builder.proxy(proxy);
    }

    builder.build().ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_connect_and_logout() {
    let config = get_test_config().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    assert!(!client.email().is_empty());

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_guard_auto_logout() {
    let config = get_test_config().expect("Test config from environment variables");

    let client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Guard will logout on drop
    let guard = client.into_guard();
    assert!(!guard.email().is_empty());

    // Explicit logout through guard
    guard.logout().await.expect("Failed to logout");
}

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_connect_displays_debug_info() {
    let config = get_test_config().expect("Test config from environment variables");

    let client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    let debug_str = format!("{:?}", client);
    assert!(debug_str.contains("ImapEmailClient"));
    assert!(debug_str.contains("email"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Find Recent Match Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_find_recent_no_match() {
    let config = get_test_config().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Search for something that won't exist
    let matcher = RegexMatcher::new(r"NONEXISTENT_PATTERN_12345").unwrap();
    let result = client
        .find_recent_match(&matcher, Duration::from_secs(60))
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.is_retryable());

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_find_recent_with_otp_matcher() {
    let config = get_test_config().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Try to find a 6-digit code in recent emails
    let matcher = OtpMatcher::six_digit();
    let result = client
        .find_recent_match(&matcher, Duration::from_secs(24 * 60))
        .await;

    // Result depends on whether there are matching emails
    match result {
        Ok(code) => {
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
        Err(e) => {
            // NoMatch is expected if no OTP emails exist
            println!("No matching OTP found (expected if no OTP emails): {}", e);
        }
    }

    client.logout().await.expect("Failed to logout");
}

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_find_recent_with_url_matcher() {
    let config = get_test_config().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Try to find URLs from common domains
    let matcher = UrlMatcher::new("github.com");
    let result = client
        .find_recent_match(&matcher, Duration::from_secs(24 * 60))
        .await;

    match result {
        Ok(url) => {
            assert!(url.starts_with("http"));
            assert!(url.contains("github.com"));
        }
        Err(e) => {
            println!("No GitHub URL found (expected if no such emails): {}", e);
        }
    }

    client.logout().await.expect("Failed to logout");
}

// ─────────────────────────────────────────────────────────────────────────────
// Wait For Match Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_wait_for_match_timeout() {
    let config =
        get_test_config_with_short_timeout().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Wait for something that won't arrive
    let matcher = RegexMatcher::new(r"WILL_NEVER_MATCH_XYZ123").unwrap();
    let result = client.wait_for_match(&matcher).await;

    assert!(result.is_err());
    let err = result.unwrap_err();

    // WaitTimeout is not retryable
    assert!(!err.is_retryable());

    client.logout().await.expect("Failed to logout");
}

// ─────────────────────────────────────────────────────────────────────────────
// Matcher Tests (Unit-style, but with real client context)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires real IMAP server"]
async fn test_custom_closure_matcher() {
    let config = get_test_config().expect("Test config from environment variables");

    let mut client = ImapEmailClient::connect(config)
        .await
        .expect("Failed to connect");

    // Custom matcher that looks for "Subject:" lines
    let matcher = ClosureMatcher::new(
        |text| {
            text.lines()
                .find(|line| line.to_lowercase().contains("subject"))
                .map(|line| Cow::Owned(line.to_string()))
        },
        "subject line finder",
    );

    let result = client
        .find_recent_match(&matcher, Duration::from_secs(24 * 60))
        .await;

    match result {
        Ok(subject) => {
            println!("Found subject: {}", subject);
        }
        Err(e) => {
            println!("No subject found: {}", e);
        }
    }

    client.logout().await.expect("Failed to logout");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error Handling Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires intentionally wrong credentials"]
async fn test_invalid_credentials() {
    let config = ImapConfig::builder()
        .email("test@gmail.com")
        .password("wrong-password")
        .build()
        .expect("valid config structure");

    let result = ImapEmailClient::connect(config).await;

    assert!(result.is_err());
    let err = result.unwrap_err();

    // Authentication errors are retryable (could be temporary server issue)
    println!("Connection error: {}", err);
    println!("Category: {}", err.category());
}

#[tokio::test]
async fn test_invalid_email_format() {
    let result = ImapConfig::builder()
        .email("not-an-email")
        .password("password")
        .build();

    assert!(result.is_err());
}

#[tokio::test]
async fn test_missing_required_fields() {
    // Missing email
    let result = ImapConfig::builder().password("password").build();
    assert!(result.is_err());

    // Missing password
    let result = ImapConfig::builder().email("test@example.com").build();
    assert!(result.is_err());
}
