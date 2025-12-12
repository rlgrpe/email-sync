//! Configuration for IMAP email client.
//!
//! Use [`ImapConfigBuilder`] to create a configuration with sensible defaults:
//!
//! ```
//! use email_sync::ImapConfig;
//!
//! let config = ImapConfig::builder()
//!     .email("user@example.com")
//!     .password("app-password")
//!     .build()
//!     .expect("valid config");
//! ```

use crate::error::{Error, Result};
use crate::known_servers::ServerRegistry;
use crate::proxy::Socks5Proxy;
use email_address::EmailAddress;
use secrecy::{ExposeSecret, SecretString};
use std::time::Duration;

/// Configuration for connecting to an IMAP server.
///
/// Create using [`ImapConfig::builder()`].
///
/// Note: The `password` field is stored as a [`SecretString`] to prevent
/// accidental logging of sensitive credentials. The `email` field is stored
/// as a validated [`EmailAddress`] type.
#[derive(Clone)]
pub struct ImapConfig {
    /// Email address (used for login and IMAP server discovery).
    /// Stored as a validated `EmailAddress` type.
    email: EmailAddress,
    /// Email password or app-specific password (protected from accidental logging).
    password: SecretString,
    /// IMAP server hostname (auto-discovered from email domain if not set).
    pub imap_host: Option<String>,
    /// IMAP server port (default: 993 for IMAPS).
    pub imap_port: u16,
    /// Optional SOCKS5 proxy for connection.
    pub proxy: Option<Socks5Proxy>,
    /// Timeout configuration.
    pub timeouts: TimeoutConfig,
    /// Polling configuration for waiting operations.
    pub polling: PollingConfig,
}

impl std::fmt::Debug for ImapConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImapConfig")
            .field("email", &self.email.as_str())
            .field("password", &"[REDACTED]")
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("proxy", &self.proxy)
            .field("timeouts", &self.timeouts)
            .field("polling", &self.polling)
            .finish()
    }
}

impl ImapConfig {
    /// Returns the email address as a string slice.
    #[must_use]
    pub fn email(&self) -> &str {
        self.email.as_str()
    }

    /// Returns a reference to the validated email address.
    #[must_use]
    pub fn email_address(&self) -> &EmailAddress {
        &self.email
    }

    /// Returns the password as a string slice.
    ///
    /// Use this method when you need to pass the password to authentication.
    /// The password is intentionally not directly accessible to prevent accidental logging.
    #[must_use]
    pub fn password(&self) -> &str {
        self.password.expose_secret()
    }
}

/// Timeout configuration for various operations.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Timeout for establishing TCP/TLS connection.
    pub connect: Duration,
    /// Timeout for IMAP authentication.
    pub auth: Duration,
    /// Timeout for selecting a mailbox.
    pub select: Duration,
    /// Timeout for fetching UIDs.
    pub uid_fetch: Duration,
    /// Timeout for fetching message content.
    pub message_fetch: Duration,
    /// Timeout for logout operation.
    pub logout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(30),
            auth: Duration::from_secs(30),
            select: Duration::from_secs(10),
            uid_fetch: Duration::from_secs(10),
            message_fetch: Duration::from_secs(30),
            logout: Duration::from_secs(5),
        }
    }
}

/// Polling configuration for wait operations.
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// Interval between polling attempts when waiting for email.
    pub interval: Duration,
    /// Maximum time to wait for matching email.
    pub max_wait: Duration,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(2),
            max_wait: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl ImapConfig {
    /// Creates a new configuration builder.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::ImapConfig;
    ///
    /// let config = ImapConfig::builder()
    ///     .email("user@gmail.com")
    ///     .password("app-password")
    ///     .build()
    ///     .expect("valid config");
    /// ```
    #[must_use]
    pub fn builder() -> ImapConfigBuilder {
        ImapConfigBuilder::default()
    }

    /// Returns the effective IMAP host, either explicitly configured or derived from email domain.
    #[must_use]
    pub fn effective_imap_host(&self) -> String {
        if let Some(host) = &self.imap_host {
            host.clone()
        } else {
            crate::known_servers::discover_imap_host(self.email.as_str())
        }
    }

    /// Returns the full IMAP server address as "host:port".
    #[must_use]
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.effective_imap_host(), self.imap_port)
    }
}

/// Validates an email address format.
///
/// Returns the validated `EmailAddress` if valid, or an error if invalid.
fn validate_email(email: &str) -> Result<EmailAddress> {
    EmailAddress::parse_with_options(email, email_address::Options::default()).map_err(|_| {
        Error::InvalidEmailFormat {
            email: email.to_string(),
        }
    })
}

/// Builder for [`ImapConfig`].
#[derive(Debug, Default)]
pub struct ImapConfigBuilder {
    email: Option<String>,
    password: Option<String>,
    imap_host: Option<String>,
    imap_port: Option<u16>,
    proxy: Option<Socks5Proxy>,
    timeouts: Option<TimeoutConfig>,
    polling: Option<PollingConfig>,
    server_registry: Option<ServerRegistry>,
}

impl ImapConfigBuilder {
    /// Sets the email address (required).
    ///
    /// The email domain is used to auto-discover the IMAP server if not explicitly set.
    #[must_use]
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Sets the password (required).
    ///
    /// For Gmail/Outlook, use an app-specific password.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Sets the IMAP server hostname explicitly.
    ///
    /// If not set, the server is auto-discovered from the email domain.
    #[must_use]
    pub fn imap_host(mut self, host: impl Into<String>) -> Self {
        self.imap_host = Some(host.into());
        self
    }

    /// Sets the IMAP server port.
    ///
    /// Default is 993 (IMAPS with TLS).
    #[must_use]
    pub fn imap_port(mut self, port: u16) -> Self {
        self.imap_port = Some(port);
        self
    }

    /// Sets a custom server registry for IMAP host discovery.
    ///
    /// The registry is used during [`build()`](Self::build) to resolve the IMAP host
    /// if no explicit [`imap_host`](Self::imap_host) is set.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::{ImapConfig, ServerRegistry};
    ///
    /// let mut registry = ServerRegistry::with_defaults();
    /// registry.register("mycompany.com", "mail.internal.mycompany.com");
    ///
    /// let config = ImapConfig::builder()
    ///     .email("user@mycompany.com")
    ///     .password("secret")
    ///     .server_registry(registry)
    ///     .build()
    ///     .expect("valid config");
    ///
    /// assert_eq!(config.effective_imap_host(), "mail.internal.mycompany.com");
    /// ```
    #[must_use]
    pub fn server_registry(mut self, registry: ServerRegistry) -> Self {
        self.server_registry = Some(registry);
        self
    }

    /// Sets a SOCKS5 proxy for the connection.
    #[must_use]
    pub fn proxy(mut self, proxy: Socks5Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Sets timeout configuration.
    #[must_use]
    pub fn timeouts(mut self, timeouts: TimeoutConfig) -> Self {
        self.timeouts = Some(timeouts);
        self
    }

    /// Sets the connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.timeouts
            .get_or_insert_with(TimeoutConfig::default)
            .connect = timeout;
        self
    }

    /// Sets the authentication timeout.
    #[must_use]
    pub fn auth_timeout(mut self, timeout: Duration) -> Self {
        self.timeouts
            .get_or_insert_with(TimeoutConfig::default)
            .auth = timeout;
        self
    }

    /// Sets polling configuration.
    #[must_use]
    pub fn polling(mut self, polling: PollingConfig) -> Self {
        self.polling = Some(polling);
        self
    }

    /// Sets the polling interval for wait operations.
    #[must_use]
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.polling
            .get_or_insert_with(PollingConfig::default)
            .interval = interval;
        self
    }

    /// Sets the maximum wait time for email operations.
    #[must_use]
    pub fn max_wait(mut self, max_wait: Duration) -> Self {
        self.polling
            .get_or_insert_with(PollingConfig::default)
            .max_wait = max_wait;
        self
    }

    /// Builds the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or invalid.
    pub fn build(self) -> Result<ImapConfig> {
        let email_raw = self.email.ok_or_else(|| Error::InvalidConfig {
            message: "email is required".into(),
        })?;

        // Validate email format using email_address crate
        let email = validate_email(&email_raw)?;

        let password_raw = self.password.ok_or_else(|| Error::InvalidConfig {
            message: "password is required".into(),
        })?;

        // Resolve IMAP host: explicit > registry > default discovery
        let imap_host = self.imap_host.or_else(|| {
            self.server_registry
                .map(|registry| registry.discover(email.as_str()).into_owned())
        });

        Ok(ImapConfig {
            email,
            password: SecretString::from(password_raw),
            imap_host,
            imap_port: self.imap_port.unwrap_or(993),
            proxy: self.proxy,
            timeouts: self.timeouts.unwrap_or_default(),
            polling: self.polling.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_minimal() {
        let config = ImapConfig::builder()
            .email("user@example.com")
            .password("secret")
            .build()
            .unwrap();

        assert_eq!(config.email(), "user@example.com");
        assert_eq!(config.password(), "secret");
        assert_eq!(config.imap_port, 993);
        assert!(config.proxy.is_none());
    }

    #[test]
    fn test_builder_full() {
        let config = ImapConfig::builder()
            .email("user@example.com")
            .password("secret")
            .imap_host("mail.example.com")
            .imap_port(994)
            .proxy(Socks5Proxy::new("proxy.local", 1080))
            .connect_timeout(Duration::from_secs(60))
            .poll_interval(Duration::from_secs(5))
            .build()
            .unwrap();

        assert_eq!(config.imap_host, Some("mail.example.com".into()));
        assert_eq!(config.imap_port, 994);
        assert!(config.proxy.is_some());
        assert_eq!(config.timeouts.connect, Duration::from_secs(60));
        assert_eq!(config.polling.interval, Duration::from_secs(5));
    }

    #[test]
    fn test_builder_missing_email() {
        let result = ImapConfig::builder().password("secret").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_missing_password() {
        let result = ImapConfig::builder().email("user@example.com").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_invalid_email() {
        let result = ImapConfig::builder()
            .email("invalid-email")
            .password("secret")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_server_address() {
        let config = ImapConfig::builder()
            .email("user@example.com")
            .password("secret")
            .imap_host("mail.example.com")
            .imap_port(993)
            .build()
            .unwrap();

        assert_eq!(config.server_address(), "mail.example.com:993");
    }

    #[test]
    fn test_password_not_in_debug() {
        let config = ImapConfig::builder()
            .email("user@example.com")
            .password("super-secret-password")
            .build()
            .unwrap();

        let debug_str = format!("{config:?}");
        assert!(!debug_str.contains("super-secret-password"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_builder_with_server_registry() {
        let mut registry = ServerRegistry::new();
        registry.register("mycompany.com", "mail.internal.mycompany.com");

        let config = ImapConfig::builder()
            .email("user@mycompany.com")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "mail.internal.mycompany.com");
    }

    #[test]
    fn test_builder_explicit_host_overrides_registry() {
        let mut registry = ServerRegistry::new();
        registry.register("mycompany.com", "mail.internal.mycompany.com");

        let config = ImapConfig::builder()
            .email("user@mycompany.com")
            .password("secret")
            .imap_host("custom.host.com")
            .server_registry(registry)
            .build()
            .unwrap();

        // Explicit host takes precedence
        assert_eq!(config.effective_imap_host(), "custom.host.com");
    }

    #[test]
    fn test_builder_registry_with_defaults() {
        // Registry with defaults should resolve known providers
        let registry = ServerRegistry::with_defaults();

        let config = ImapConfig::builder()
            .email("user@gmail.com")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "imap.gmail.com");
    }

    #[test]
    fn test_builder_registry_unknown_domain_fallback() {
        // Registry should fall back to imap.{domain} for unknown domains
        let registry = ServerRegistry::with_defaults();

        let config = ImapConfig::builder()
            .email("user@unknowndomain123.org")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "imap.unknowndomain123.org");
    }

    #[test]
    fn test_builder_empty_registry_fallback() {
        // Empty registry without defaults should still produce fallback
        let registry = ServerRegistry::new();

        let config = ImapConfig::builder()
            .email("user@example.com")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "imap.example.com");
    }

    #[test]
    fn test_builder_registry_case_insensitive() {
        let mut registry = ServerRegistry::new();
        registry.register("MyCompany.COM", "mail.mycompany.com");

        let config = ImapConfig::builder()
            .email("user@MYCOMPANY.com")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "mail.mycompany.com");
    }

    #[test]
    fn test_builder_registry_overrides_builtin() {
        // Custom mapping should override built-in defaults
        let mut registry = ServerRegistry::with_defaults();
        registry.register("gmail.com", "custom-gmail-proxy.internal");

        let config = ImapConfig::builder()
            .email("user@gmail.com")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "custom-gmail-proxy.internal");
    }

    #[test]
    fn test_builder_no_registry_uses_default_discovery() {
        // Without registry, should use built-in discover_imap_host
        let config = ImapConfig::builder()
            .email("user@gmail.com")
            .password("secret")
            .build()
            .unwrap();

        assert_eq!(config.effective_imap_host(), "imap.gmail.com");
    }

    #[test]
    fn test_builder_registry_multiple_domains() {
        let mut registry = ServerRegistry::new();
        registry.register_many([
            ("corp.com", "mail.corp.internal"),
            ("partner.org", "imap.partner.org"),
            ("vendor.net", "mail.vendor.net"),
        ]);

        let config1 = ImapConfig::builder()
            .email("alice@corp.com")
            .password("secret")
            .server_registry(registry.clone())
            .build()
            .unwrap();

        let config2 = ImapConfig::builder()
            .email("bob@partner.org")
            .password("secret")
            .server_registry(registry.clone())
            .build()
            .unwrap();

        let config3 = ImapConfig::builder()
            .email("carol@vendor.net")
            .password("secret")
            .server_registry(registry)
            .build()
            .unwrap();

        assert_eq!(config1.effective_imap_host(), "mail.corp.internal");
        assert_eq!(config2.effective_imap_host(), "imap.partner.org");
        assert_eq!(config3.effective_imap_host(), "mail.vendor.net");
    }
}
