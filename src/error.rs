//! Error types for the email-sync crate.
//!
//! All errors implement [`std::error::Error`] and provide context about what went wrong.
//! Errors are categorized by their retryability - see [`Error::is_retryable`].

use std::time::Duration;
use thiserror::Error;

/// Result type alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during email operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    // ─────────────────────────────────────────────────────────────────────────
    // Configuration / validation errors (NOT retryable)
    // ─────────────────────────────────────────────────────────────────────────
    /// Invalid email address format.
    #[error("invalid email format: {email}")]
    InvalidEmailFormat {
        /// The invalid email address.
        email: String,
    },

    /// Invalid configuration provided.
    #[error("invalid configuration: {message}")]
    InvalidConfig {
        /// Description of the configuration error.
        message: String,
    },

    /// Invalid DNS name for TLS.
    #[error("invalid DNS name for host '{host}'")]
    InvalidDnsName {
        /// The invalid hostname.
        host: String,
        /// The underlying DNS name error.
        #[source]
        source: rustls::client::InvalidDnsNameError,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Network / connection errors (RETRYABLE)
    // ─────────────────────────────────────────────────────────────────────────
    /// Failed to establish TCP connection.
    #[error("failed to connect to {target}")]
    TcpConnect {
        /// The target address that failed.
        target: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to establish TLS connection.
    #[error("failed to establish TLS connection to {target}")]
    TlsConnect {
        /// The target address that failed.
        target: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to connect via SOCKS5 proxy.
    #[error("failed to connect via SOCKS5 proxy {proxy_host} to {target}")]
    Socks5Connect {
        /// The SOCKS5 proxy hostname.
        proxy_host: String,
        /// The target address.
        target: String,
        /// The underlying SOCKS5 error.
        #[source]
        source: tokio_socks::Error,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Timeout errors (mixed retryability)
    // ─────────────────────────────────────────────────────────────────────────
    /// Connection timeout.
    #[error("connection timeout to {target} after {timeout:?}")]
    ConnectTimeout {
        /// The target address.
        target: String,
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Authentication timeout.
    #[error("authentication timeout for {email} after {timeout:?}")]
    AuthTimeout {
        /// The email address used for authentication.
        email: String,
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Mailbox selection timeout.
    #[error("mailbox selection timeout for '{mailbox}' after {timeout:?}")]
    SelectTimeout {
        /// The mailbox name.
        mailbox: String,
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// UID fetch timeout.
    #[error("UID fetch timeout after {timeout:?}")]
    UidFetchTimeout {
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Message fetch timeout.
    #[error("message fetch timeout for UID range {uid_range} after {timeout:?}")]
    FetchTimeout {
        /// The UID range being fetched.
        uid_range: String,
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Timeout waiting for matching email.
    #[error("timeout waiting for matching email after {timeout:?}")]
    WaitTimeout {
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    /// Logout timeout (not critical).
    #[error("logout timeout after {timeout:?}")]
    LogoutTimeout {
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // IMAP protocol errors (RETRYABLE - could be transient server issues)
    // ─────────────────────────────────────────────────────────────────────────
    /// IMAP login failed.
    #[error("IMAP login failed for {email}")]
    ImapLogin {
        /// The email address used for login.
        email: String,
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// Failed to select mailbox.
    #[error("failed to select mailbox '{mailbox}'")]
    SelectMailbox {
        /// The mailbox name.
        mailbox: String,
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// IMAP NOOP failed.
    #[error("IMAP NOOP command failed")]
    ImapNoop {
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// IMAP search failed.
    #[error("IMAP search failed")]
    ImapSearch {
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// IMAP fetch failed.
    #[error("IMAP fetch failed for UID range {uid_range}")]
    ImapFetch {
        /// The UID range that failed.
        uid_range: String,
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// Failed to fetch message from stream.
    #[error("failed to fetch message from stream")]
    FetchMessage {
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    /// IMAP logout failed.
    #[error("IMAP logout failed")]
    ImapLogout {
        /// The underlying IMAP error.
        #[source]
        source: async_imap::error::Error,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Email parsing errors (NOT retryable - malformed content won't change)
    // ─────────────────────────────────────────────────────────────────────────
    /// Failed to parse email message.
    #[error("failed to parse email")]
    ParseEmail {
        /// The underlying parse error.
        #[source]
        source: mailparse::MailParseError,
    },

    /// Failed to extract email body.
    #[error("failed to extract email body")]
    ExtractBody {
        /// The underlying parse error.
        #[source]
        source: mailparse::MailParseError,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Search result errors (NOT retryable)
    // ─────────────────────────────────────────────────────────────────────────
    /// No matching email found.
    #[error("no matching email found")]
    NoMatch,
}

impl Error {
    /// Returns `true` if this error represents a transient failure that might succeed on retry.
    ///
    /// Use this to implement retry logic:
    ///
    /// ```ignore
    /// if error.is_retryable() {
    ///     // Backoff and retry
    /// } else {
    ///     // Fail permanently
    /// }
    /// ```
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            // RETRYABLE errors: network, connection timeouts, IMAP operations
            Error::TcpConnect { .. }
            | Error::TlsConnect { .. }
            | Error::Socks5Connect { .. }
            | Error::ConnectTimeout { .. }
            | Error::AuthTimeout { .. }
            | Error::SelectTimeout { .. }
            | Error::UidFetchTimeout { .. }
            | Error::FetchTimeout { .. }
            | Error::ImapLogin { .. }
            | Error::SelectMailbox { .. }
            | Error::ImapNoop { .. }
            | Error::ImapSearch { .. }
            | Error::ImapFetch { .. }
            | Error::FetchMessage { .. } => true,

            // NOT retryable: config errors, wait/logout timeouts, parsing, no match
            Error::InvalidEmailFormat { .. }
            | Error::InvalidConfig { .. }
            | Error::InvalidDnsName { .. }
            | Error::WaitTimeout { .. }
            | Error::LogoutTimeout { .. }
            | Error::ImapLogout { .. }
            | Error::ParseEmail { .. }
            | Error::ExtractBody { .. }
            | Error::NoMatch => false,
        }
    }

    /// Returns the error category for metrics/logging purposes.
    #[must_use]
    pub fn category(&self) -> ErrorCategory {
        match self {
            Error::InvalidEmailFormat { .. }
            | Error::InvalidConfig { .. }
            | Error::InvalidDnsName { .. } => ErrorCategory::Configuration,

            Error::TcpConnect { .. } | Error::TlsConnect { .. } | Error::Socks5Connect { .. } => {
                ErrorCategory::Network
            }

            Error::ConnectTimeout { .. }
            | Error::AuthTimeout { .. }
            | Error::SelectTimeout { .. }
            | Error::UidFetchTimeout { .. }
            | Error::FetchTimeout { .. }
            | Error::WaitTimeout { .. }
            | Error::LogoutTimeout { .. } => ErrorCategory::Timeout,

            Error::ImapLogin { .. }
            | Error::SelectMailbox { .. }
            | Error::ImapNoop { .. }
            | Error::ImapSearch { .. }
            | Error::ImapFetch { .. }
            | Error::FetchMessage { .. }
            | Error::ImapLogout { .. } => ErrorCategory::Protocol,

            Error::ParseEmail { .. } | Error::ExtractBody { .. } => ErrorCategory::Parse,

            Error::NoMatch => ErrorCategory::NotFound,
        }
    }
}

/// Error categories for metrics and logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Configuration or validation errors.
    Configuration,
    /// Network connectivity errors.
    Network,
    /// Timeout errors.
    Timeout,
    /// IMAP protocol errors.
    Protocol,
    /// Email parsing errors.
    Parse,
    /// No matching content found.
    NotFound,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCategory::Configuration => write!(f, "configuration"),
            ErrorCategory::Network => write!(f, "network"),
            ErrorCategory::Timeout => write!(f, "timeout"),
            ErrorCategory::Protocol => write!(f, "protocol"),
            ErrorCategory::Parse => write!(f, "parse"),
            ErrorCategory::NotFound => write!(f, "not_found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_classification() {
        // Configuration errors are not retryable
        let err = Error::InvalidEmailFormat {
            email: "bad".into(),
        };
        assert!(!err.is_retryable());

        // Network errors are retryable
        let err = Error::TcpConnect {
            target: "imap.example.com:993".into(),
            source: std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"),
        };
        assert!(err.is_retryable());

        // Wait timeout is not retryable (we already waited)
        let err = Error::WaitTimeout {
            timeout: Duration::from_secs(30),
        };
        assert!(!err.is_retryable());

        // NoMatch is not retryable
        let err = Error::NoMatch;
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_error_categories() {
        let err = Error::InvalidEmailFormat {
            email: "bad".into(),
        };
        assert_eq!(err.category(), ErrorCategory::Configuration);

        let err = Error::ConnectTimeout {
            target: "imap.example.com:993".into(),
            timeout: Duration::from_secs(10),
        };
        assert_eq!(err.category(), ErrorCategory::Timeout);

        let err = Error::NoMatch;
        assert_eq!(err.category(), ErrorCategory::NotFound);
    }
}
