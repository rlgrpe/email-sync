//! Example: Connect through a SOCKS5 proxy.
//!
//! This example shows how to route IMAP connections through a SOCKS5 proxy,
//! which is useful for:
//! - Bypassing network restrictions
//! - Testing from different geographic locations
//! - Privacy/anonymity requirements
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! export PROXY_HOST="proxy.example.com"
//! export PROXY_PORT="1080"
//! # Optional: for authenticated proxies
//! export PROXY_USER="username"
//! export PROXY_PASS="password"
//!
//! cargo run --example with_proxy
//! ```

use email_sync::matcher::OtpMatcher;
use email_sync::{ImapConfig, ImapEmailClient, Socks5Proxy};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> email_sync::Result<()> {
    // Email credentials
    let email = env::var("EMAIL_ADDRESS").expect("EMAIL_ADDRESS environment variable required");
    let password =
        env::var("EMAIL_PASSWORD").expect("EMAIL_PASSWORD environment variable required");

    // Proxy configuration
    let proxy_host = env::var("PROXY_HOST").expect("PROXY_HOST environment variable required");
    let proxy_port: u16 = env::var("PROXY_PORT")
        .expect("PROXY_PORT environment variable required")
        .parse()
        .expect("PROXY_PORT must be a valid port number");

    // Create proxy (with optional authentication)
    let proxy = match (env::var("PROXY_USER").ok(), env::var("PROXY_PASS").ok()) {
        (Some(user), Some(pass)) => {
            println!(
                "Using authenticated SOCKS5 proxy at {}:{}",
                proxy_host, proxy_port
            );
            Socks5Proxy::with_auth(&proxy_host, proxy_port, user, pass)
        }
        _ => {
            println!("Using SOCKS5 proxy at {}:{}", proxy_host, proxy_port);
            Socks5Proxy::new(&proxy_host, proxy_port)
        }
    };

    println!("Connecting to IMAP server for {} via proxy...", email);

    // Build configuration with proxy
    let config = ImapConfig::builder()
        .email(&email)
        .password(password)
        .proxy(proxy)
        // Increase timeouts for proxy connections
        .connect_timeout(Duration::from_secs(60))
        .auth_timeout(Duration::from_secs(60))
        .build()?;

    // Connect through the proxy
    let mut client = ImapEmailClient::connect(config).await?;

    println!("Connected via proxy!");
    println!("IMAP Host: {}", client.imap_host());

    // Search for recent OTP codes
    println!("\nSearching for OTP codes in recent emails...");
    match client
        .find_recent_match(&OtpMatcher::six_digit(), Duration::from_secs(3600))
        .await
    {
        Ok(code) => println!("Found OTP: {}", code),
        Err(e) => println!("No OTP found: {}", e),
    }

    client.logout().await?;

    println!("\nDisconnected from proxy.");
    Ok(())
}
