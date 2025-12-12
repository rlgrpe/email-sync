//! SOCKS5 proxy configuration for IMAP connections.
//!
//! This module provides a simple, self-contained proxy configuration that can be
//! used to route IMAP connections through a SOCKS5 proxy.
//!
//! # Example
//!
//! ```
//! use email_sync::Socks5Proxy;
//!
//! // Without authentication
//! let proxy = Socks5Proxy::new("proxy.example.com", 1080);
//!
//! // With authentication
//! let proxy = Socks5Proxy::with_auth("proxy.example.com", 1080, "username", "password");
//! ```

/// SOCKS5 proxy configuration.
#[derive(Debug, Clone)]
pub struct Socks5Proxy {
    /// Proxy server hostname or IP address.
    pub host: String,
    /// Proxy server port.
    pub port: u16,
    /// Optional authentication credentials.
    pub auth: Option<ProxyAuth>,
}

/// Authentication credentials for SOCKS5 proxy.
#[derive(Debug, Clone)]
pub struct ProxyAuth {
    /// Username for proxy authentication.
    pub username: String,
    /// Password for proxy authentication.
    pub password: String,
}

impl Socks5Proxy {
    /// Creates a new SOCKS5 proxy configuration without authentication.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::Socks5Proxy;
    ///
    /// let proxy = Socks5Proxy::new("192.168.1.1", 1080);
    /// ```
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            auth: None,
        }
    }

    /// Creates a new SOCKS5 proxy configuration with authentication.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::Socks5Proxy;
    ///
    /// let proxy = Socks5Proxy::with_auth("192.168.1.1", 1080, "user", "pass");
    /// ```
    #[must_use]
    pub fn with_auth(
        host: impl Into<String>,
        port: u16,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            auth: Some(ProxyAuth {
                username: username.into(),
                password: password.into(),
            }),
        }
    }

    /// Returns the proxy address as "host:port".
    #[must_use]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Returns `true` if this proxy requires authentication.
    #[must_use]
    pub fn requires_auth(&self) -> bool {
        self.auth.is_some()
    }
}

impl std::fmt::Display for Socks5Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.auth {
            Some(auth) => write!(
                f,
                "socks5://{}:***@{}:{}",
                auth.username, self.host, self.port
            ),
            None => write!(f, "socks5://{}:{}", self.host, self.port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_without_auth() {
        let proxy = Socks5Proxy::new("192.168.1.1", 1080);
        assert_eq!(proxy.host, "192.168.1.1");
        assert_eq!(proxy.port, 1080);
        assert!(proxy.auth.is_none());
        assert!(!proxy.requires_auth());
        assert_eq!(proxy.address(), "192.168.1.1:1080");
    }

    #[test]
    fn test_proxy_with_auth() {
        let proxy = Socks5Proxy::with_auth("proxy.example.com", 1080, "user", "pass");
        assert_eq!(proxy.host, "proxy.example.com");
        assert_eq!(proxy.port, 1080);
        assert!(proxy.auth.is_some());
        assert!(proxy.requires_auth());

        let auth = proxy.auth.as_ref().unwrap();
        assert_eq!(auth.username, "user");
        assert_eq!(auth.password, "pass");
    }

    #[test]
    fn test_display_masks_password() {
        let proxy = Socks5Proxy::with_auth("proxy.example.com", 1080, "user", "secret");
        let display = proxy.to_string();
        assert!(display.contains("***"));
        assert!(!display.contains("secret"));
    }
}
