# email-sync

Async IMAP email client for monitoring mailboxes and extracting content using pattern matching.

> **Disclaimer**: This library is provided as-is. I am not obligated to maintain it, fix bugs, or add features. If you
> want to contribute improvements, please submit a pull request.

## Features

- **Async/await** - Built on Tokio for efficient async I/O
- **Pattern matching** - Extract OTP codes, URLs, tokens, or custom patterns from emails
- **Auto-discovery** - Automatically discovers IMAP servers for common email providers
- **SOCKS5 proxy support** - Route connections through SOCKS5 proxies
- **Observability** - Structured tracing with optional OpenTelemetry integration
- **Error classification** - Errors indicate whether they're retryable for robust retry logic

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
email-sync = { git = "https://github.com/rlgrpe/email-sync.git", tag = "v0.1.0" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use email_sync::{ImapConfig, ImapEmailClient};
use email_sync::matcher::OtpMatcher;

#[tokio::main]
async fn main() -> email_sync::Result<()> {
    // Configure the client
    let config = ImapConfig::builder()
        .email("user@gmail.com")
        .password("app-password")  // Use app-specific password for Gmail
        .build()?;

    // Connect to IMAP server
    let mut client = ImapEmailClient::connect(config).await?;

    // Wait for a 6-digit OTP code
    let otp = client.wait_for_match(&OtpMatcher::six_digit()).await?;
    println!("Got OTP: {}", otp);

    // Clean up
    client.logout().await?;
    Ok(())
}
```

## Usage

### Basic Configuration

```rust
use email_sync::ImapConfig;

// Minimal configuration (IMAP host auto-discovered from email domain)
let config = ImapConfig::builder()
.email("user@gmail.com")
.password("app-password")
.build() ?;

// Full configuration
let config = ImapConfig::builder()
.email("user@example.com")
.password("password")
.imap_host("mail.example.com")  // Override auto-discovery
.imap_port(993)
.connect_timeout(Duration::from_secs(30))
.poll_interval(Duration::from_secs(2))
.max_wait(Duration::from_secs(300))
.build() ?;
```

### Pattern Matchers

#### OTP Codes

```rust
use email_sync::matcher::OtpMatcher;

// 6-digit OTP (most common)
let matcher = OtpMatcher::six_digit();

// Custom digit count
let matcher = OtpMatcher::n_digit(4);  // 4-digit PIN
```

#### URLs

```rust
use email_sync::matcher::UrlMatcher;

// Extract URLs from a specific domain
let matcher = UrlMatcher::new("example.com");
```

#### Custom Regex

```rust
use email_sync::matcher::RegexMatcher;

// Extract first capture group
let matcher = RegexMatcher::new(r"token=([a-f0-9]{32})") ?;

// With custom description (shown in logs)
let matcher = RegexMatcher::with_description(
r"order[_-]?id[=:]\s*(\d+)",
"Order ID"
) ?;
```

#### Closure-based Matchers

```rust
use email_sync::matcher::ClosureMatcher;

let matcher = ClosureMatcher::new(
| text| {
text.lines()
.find( | line| line.starts_with("CODE:"))
.map( | line | line.trim_start_matches("CODE:").trim().to_string())
},
"code line extractor"
);
```

### Finding vs Waiting

```rust
use std::time::Duration;

// Wait for NEW emails (polls until match or timeout)
let code = client.wait_for_match( & matcher).await?;

// Search EXISTING recent emails (no polling)
let code = client.find_recent_match( & matcher, Duration::from_secs(3600)).await?;
```

### SOCKS5 Proxy

```rust
use email_sync::{ImapConfig, Socks5Proxy};

// Without authentication
let proxy = Socks5Proxy::new("proxy.example.com", 1080);

// With authentication
let proxy = Socks5Proxy::with_auth("proxy.example.com", 1080, "user", "pass");

let config = ImapConfig::builder()
.email("user@gmail.com")
.password("app-password")
.proxy(proxy)
.build() ?;
```

### RAII Guard for Automatic Cleanup

```rust
let client = ImapEmailClient::connect(config).await?;
let mut guard = client.into_guard();  // Will logout on drop

let code = guard.wait_for_match( & OtpMatcher::six_digit()).await?;
// Guard automatically logs out when dropped, even on early return or panic
```

### Error Handling

```rust
use email_sync::{Error, ErrorCategory};

match client.wait_for_match( & matcher).await {
Ok(code) => println!("Found: {}", code),
Err(e) => {
// Check if error is transient (can retry)
if e.is_retryable() {
println ! ("Transient error, retrying: {}", e);
} else {
println ! ("Permanent error: {}", e);
}

// Categorize for metrics/logging
match e.category() {
ErrorCategory::Network => { /* connection issues */ }
ErrorCategory::Timeout => { /* operation timed out */ }
ErrorCategory::Protocol => { /* IMAP errors */ }
ErrorCategory::Parse => { /* email parsing failed */ }
ErrorCategory::Configuration => { /* invalid config */ }
ErrorCategory::NotFound => { /* no matching email */ }
}
}
}
```

## Examples

Run the examples with:

```bash
export EMAIL_ADDRESS="your@email.com"
export EMAIL_PASSWORD="your-app-password"

cargo run --example basic_otp
cargo run --example find_recent
cargo run --example custom_matcher
cargo run --example with_proxy
cargo run --example with_tracing
cargo run --example error_handling
```

| Example          | Description                                     |
|------------------|-------------------------------------------------|
| `basic_otp`      | Wait for a 6-digit OTP code                     |
| `find_recent`    | Search recent emails for various patterns       |
| `custom_matcher` | Create custom matchers with regex and closures  |
| `with_proxy`     | Connect through a SOCKS5 proxy                  |
| `with_tracing`   | Enable structured logging                       |
| `error_handling` | Implement retry logic with error classification |

## Supported Email Providers

The library auto-discovers IMAP servers for these providers:

- Gmail (`gmail.com`)
- Outlook/Hotmail (`outlook.com`, `hotmail.com`, `live.com`)
- Yahoo (`yahoo.com`)
- iCloud (`icloud.com`, `me.com`, `mac.com`)
- Mail.ru (`mail.ru`, `bk.ru`, `inbox.ru`, `list.ru`)
- Yandex (`yandex.ru`, `yandex.com`)
- AOL (`aol.com`)
- German providers (`web.de`, `gmx.de`, `t-online.de`)
- And more...

For unlisted providers, set `imap_host` explicitly or the library defaults to `imap.{domain}`.

## Features Flags

```toml
[dependencies]
email-sync = { git = "https://github.com/rlgrpe/email-sync.git", tag = "v0.1.0", features = ["observability"] }
```

| Feature         | Description                                               |
|-----------------|-----------------------------------------------------------|
| `observability` | Enables OpenTelemetry integration for distributed tracing |

## Tracing

All operations emit structured tracing spans:

- `ImapEmailClient::connect` - Connection establishment
- `ImapEmailClient::wait_for_match` - Polling for emails
- `ImapEmailClient::find_recent_match` - Searching recent emails
- `session::authenticate` - IMAP authentication
- `connection::establish_tls` - TLS handshake

Enable logging with:

```rust
tracing_subscriber::fmt()
.with_env_filter("email_sync=debug")
.init();
```

## Testing

```bash
# Unit tests
cargo test

# Integration tests (requires real IMAP server)
export EMAIL_SYNC_TEST_EMAIL="your@email.com"
export EMAIL_SYNC_TEST_PASSWORD="your-app-password"
cargo test -- --ignored
```

## License

Licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
