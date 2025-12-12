//! Example: Using custom matchers for complex extraction.
//!
//! This example demonstrates how to create custom matchers using:
//! - `RegexMatcher` for pattern-based extraction
//! - `ClosureMatcher` for arbitrary logic
//!
//! # Usage
//!
//! ```bash
//! export EMAIL_ADDRESS="your@email.com"
//! export EMAIL_PASSWORD="your-app-password"
//! cargo run --example custom_matcher
//! ```

use email_sync::matcher::{ClosureMatcher, Matcher, RegexMatcher};
use email_sync::{ImapConfig, ImapEmailClient};
use std::borrow::Cow;
use std::env;
use std::time::Duration;

/// A custom matcher that extracts order IDs in the format "ORD-XXXXX"
fn order_id_matcher() -> RegexMatcher {
    RegexMatcher::with_description(r"(ORD-\d{5,})", "Order ID (ORD-XXXXX format)")
        .expect("valid regex")
}

/// A custom matcher that extracts the first monetary amount
fn amount_matcher() -> RegexMatcher {
    RegexMatcher::with_description(r"\$(\d+(?:\.\d{2})?)", "Dollar amount").expect("valid regex")
}

/// A closure-based matcher for complex extraction logic
fn json_field_matcher(field_name: &str) -> impl Matcher {
    let field = field_name.to_string();
    ClosureMatcher::new(
        move |text| {
            // Simple JSON field extraction (not a full parser)
            let pattern = format!(r#""{}"\s*:\s*"([^"]*)""#, field);
            regex::Regex::new(&pattern)
                .ok()
                .and_then(|re| re.captures(text))
                .and_then(|caps| caps.get(1))
                .map(|m| Cow::Borrowed(m.as_str()))
        },
        format!("JSON field: {}", field_name),
    )
}

/// A matcher that extracts activation/verification links
fn activation_link_matcher() -> impl Matcher {
    ClosureMatcher::new(
        |text| {
            // Look for common activation link patterns
            let patterns = [
                r#"href="(https?://[^"]*(?:activate|verify|confirm)[^"]*)""#,
                r"(https?://\S*(?:activate|verify|confirm)\S*)",
            ];

            for pattern in patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if let Some(caps) = re.captures(text) {
                        if let Some(m) = caps.get(1) {
                            return Some(Cow::Borrowed(m.as_str()));
                        }
                    }
                }
            }
            None
        },
        "Activation/verification link",
    )
}

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

    println!("Connected! Testing custom matchers...\n");

    let max_age = Duration::from_secs(7 * 24 * 3600); // Last 7 days

    // Test order ID matcher
    println!("1. Looking for order IDs (ORD-XXXXX)...");
    match client.find_recent_match(&order_id_matcher(), max_age).await {
        Ok(order) => println!("   Found: {}", order),
        Err(_) => println!("   Not found"),
    }

    // Test amount matcher
    println!("\n2. Looking for dollar amounts...");
    match client.find_recent_match(&amount_matcher(), max_age).await {
        Ok(amount) => println!("   Found: ${}", amount),
        Err(_) => println!("   Not found"),
    }

    // Test JSON field matcher
    println!("\n3. Looking for JSON 'code' field...");
    match client
        .find_recent_match(&json_field_matcher("code"), max_age)
        .await
    {
        Ok(code) => println!("   Found: {}", code),
        Err(_) => println!("   Not found"),
    }

    // Test activation link matcher
    println!("\n4. Looking for activation/verification links...");
    match client
        .find_recent_match(&activation_link_matcher(), max_age)
        .await
    {
        Ok(link) => println!("   Found: {}", link),
        Err(_) => println!("   Not found"),
    }

    client.logout().await?;

    println!("\nDone!");
    Ok(())
}
