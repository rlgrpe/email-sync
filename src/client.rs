//! IMAP email client for monitoring and pattern matching.
//!
//! The [`ImapEmailClient`] is the main entry point for this crate. It provides
//! async methods to:
//!
//! - Wait for emails matching a pattern
//! - Find recent emails matching a pattern
//! - Poll for new emails
//!
//! # Example
//!
//! ```no_run
//! use email_sync::{ImapConfig, ImapEmailClient};
//! use email_sync::matcher::OtpMatcher;
//! use std::time::Duration;
//!
//! # async fn example() -> email_sync::Result<()> {
//! let config = ImapConfig::builder()
//!     .email("user@gmail.com")
//!     .password("app-password")
//!     .build()?;
//!
//! let mut client = ImapEmailClient::connect(config).await?;
//!
//! // Wait for an OTP code
//! let otp = client.wait_for_match(&OtpMatcher::six_digit()).await?;
//! println!("Got OTP: {}", otp);
//!
//! // Clean up
//! client.logout().await?;
//! # Ok(())
//! # }
//! ```

use crate::config::ImapConfig;
use crate::connection;
use crate::error::{Error, Result};
use crate::matcher::Matcher;
use crate::parser::{self, ExtractResult};
use crate::session::{self, AuthConfig, ImapSession};
use chrono::{NaiveDate, Utc};
use futures::StreamExt;
use std::time::{Duration, Instant};
use tracing::{debug, instrument, warn};

/// Async IMAP client for email monitoring and pattern matching.
///
/// Create using [`ImapEmailClient::connect`].
///
/// # Lifecycle
///
/// 1. Create a client with [`connect`](Self::connect)
/// 2. Use [`wait_for_match`](Self::wait_for_match) or [`find_recent_match`](Self::find_recent_match)
/// 3. Call [`logout`](Self::logout) when done (or use [`into_guard`](Self::into_guard) for RAII)
///
/// # Example
///
/// ```no_run
/// use email_sync::{ImapConfig, ImapEmailClient};
/// use email_sync::matcher::OtpMatcher;
///
/// # async fn example() -> email_sync::Result<()> {
/// let config = ImapConfig::builder()
///     .email("user@gmail.com")
///     .password("app-password")
///     .build()?;
///
/// let mut client = ImapEmailClient::connect(config).await?;
/// let code = client.wait_for_match(&OtpMatcher::six_digit()).await?;
/// client.logout().await?;
/// # Ok(())
/// # }
/// ```
pub struct ImapEmailClient {
    session: Box<ImapSession>,
    config: ImapConfig,
    start_uid: u32,
}

impl ImapEmailClient {
    /// Connects to the IMAP server and prepares for email monitoring.
    ///
    /// This establishes a TLS connection, authenticates, and selects the INBOX.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Connection cannot be established
    /// - Authentication fails
    /// - Mailbox selection fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use email_sync::{ImapConfig, ImapEmailClient};
    ///
    /// # async fn example() -> email_sync::Result<()> {
    /// let config = ImapConfig::builder()
    ///     .email("user@example.com")
    ///     .password("secret")
    ///     .build()?;
    ///
    /// let client = ImapEmailClient::connect(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "ImapEmailClient::connect",
        skip_all,
        fields(
            email = %config.email(),
            imap_host = %config.effective_imap_host(),
            proxy_enabled = config.proxy.is_some()
        )
    )]
    pub async fn connect(config: ImapConfig) -> Result<Self> {
        let mut session = Self::initialize_session(&config).await?;
        let start_uid = Self::get_initial_uid(&mut session, &config).await?;

        debug!(start_uid, "Client connected and ready");

        Ok(Self {
            session: Box::new(session),
            config,
            start_uid,
        })
    }

    /// Waits for an email matching the provided pattern.
    ///
    /// Polls the mailbox at the configured interval until a match is found
    /// or the timeout is reached.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Timeout is reached without finding a match ([`Error::WaitTimeout`])
    /// - IMAP operations fail
    ///
    /// # Example
    ///
    /// ```no_run
    /// use email_sync::{ImapConfig, ImapEmailClient};
    /// use email_sync::matcher::OtpMatcher;
    ///
    /// # async fn example() -> email_sync::Result<()> {
    /// # let config = ImapConfig::builder().email("a@b.c").password("x").build()?;
    /// let mut client = ImapEmailClient::connect(config).await?;
    /// let code = client.wait_for_match(&OtpMatcher::six_digit()).await?;
    /// println!("Got code: {}", code);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "ImapEmailClient::wait_for_match",
        skip(self, matcher),
        fields(matcher = %matcher.description())
    )]
    pub async fn wait_for_match(&mut self, matcher: &dyn Matcher) -> Result<String> {
        let timeout = self.config.polling.max_wait;
        let poll_interval = self.config.polling.interval;
        let deadline = Instant::now() + timeout;

        loop {
            if Instant::now() > deadline {
                return Err(Error::WaitTimeout { timeout });
            }

            if let Some(result) = self.check_new_emails(matcher).await? {
                return Ok(result);
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Finds a matching email among recent messages.
    ///
    /// Unlike [`wait_for_match`](Self::wait_for_match), this checks existing messages
    /// immediately without polling for new emails.
    ///
    /// # Arguments
    ///
    /// * `matcher` - The pattern to match
    /// * `max_age` - Only consider emails newer than this duration
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoMatch`] if no matching email is found.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use email_sync::{ImapConfig, ImapEmailClient};
    /// use email_sync::matcher::UrlMatcher;
    /// use std::time::Duration;
    ///
    /// # async fn example() -> email_sync::Result<()> {
    /// # let config = ImapConfig::builder().email("a@b.c").password("x").build()?;
    /// let mut client = ImapEmailClient::connect(config).await?;
    ///
    /// // Find activation link from the last 5 minutes
    /// let matcher = UrlMatcher::new("example.com");
    /// let url = client.find_recent_match(&matcher, Duration::from_secs(300)).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "ImapEmailClient::find_recent_match",
        skip(self, matcher),
        fields(
            matcher = %matcher.description(),
            max_age_secs = max_age.as_secs()
        )
    )]
    pub async fn find_recent_match(
        &mut self,
        matcher: &dyn Matcher,
        max_age: Duration,
    ) -> Result<String> {
        let since_date = Self::calculate_since_date(max_age);

        debug!(since_date = %since_date, "Searching for recent emails");

        let uids = self.search_emails_since(since_date).await?;

        if uids.is_empty() {
            return Err(Error::NoMatch);
        }

        self.find_match_in_uids(&uids, matcher).await
    }

    /// Logs out from the IMAP server.
    ///
    /// This should be called when you're done with the client.
    /// If you don't call this, the connection will be dropped without
    /// a clean logout (which is usually fine, but not ideal).
    ///
    /// # Errors
    ///
    /// Returns an error if the logout command fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use email_sync::{ImapConfig, ImapEmailClient};
    ///
    /// # async fn example() -> email_sync::Result<()> {
    /// # let config = ImapConfig::builder().email("a@b.c").password("x").build()?;
    /// let mut client = ImapEmailClient::connect(config).await?;
    /// // ... use client ...
    /// client.logout().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(name = "ImapEmailClient::logout", skip(self))]
    pub async fn logout(&mut self) -> Result<()> {
        session::logout(&mut self.session).await
    }

    /// Converts this client into a guard that logs out on drop.
    ///
    /// This is useful for ensuring cleanup in the face of early returns
    /// or panics.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use email_sync::{ImapConfig, ImapEmailClient};
    /// use email_sync::matcher::OtpMatcher;
    ///
    /// # async fn example() -> email_sync::Result<()> {
    /// # let config = ImapConfig::builder().email("a@b.c").password("x").build()?;
    /// let client = ImapEmailClient::connect(config).await?;
    /// let mut guard = client.into_guard();
    ///
    /// let code = guard.wait_for_match(&OtpMatcher::six_digit()).await?;
    /// // Guard will logout when dropped
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn into_guard(self) -> ImapEmailClientGuard {
        ImapEmailClientGuard { inner: Some(self) }
    }

    /// Returns the email address used for this connection.
    #[must_use]
    pub fn email(&self) -> &str {
        self.config.email()
    }

    /// Returns the IMAP host used for this connection.
    #[must_use]
    pub fn imap_host(&self) -> String {
        self.config.effective_imap_host()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Initializes IMAP session with connection, authentication, and mailbox selection.
    async fn initialize_session(config: &ImapConfig) -> Result<ImapSession> {
        let imap_host = config.effective_imap_host();
        let target_addr = config.server_address();
        let timeouts = &config.timeouts;

        // Establish TLS connection
        let tls_stream = tokio::time::timeout(
            timeouts.connect,
            connection::establish_tls_connection(&imap_host, &target_addr, config.proxy.as_ref()),
        )
        .await
        .map_err(|_| Error::ConnectTimeout {
            target: target_addr.clone(),
            timeout: timeouts.connect,
        })??;

        debug!("TLS connection established");

        // Authenticate
        let auth_config = AuthConfig {
            email: config.email(),
            password: config.password(),
        };

        let mut session = tokio::time::timeout(
            timeouts.auth,
            session::authenticate(tls_stream, &auth_config),
        )
        .await
        .map_err(|_| Error::AuthTimeout {
            email: config.email().to_string(),
            timeout: timeouts.auth,
        })??;

        debug!("Authenticated");

        // Select INBOX
        tokio::time::timeout(
            timeouts.select,
            session::select_mailbox(&mut session, "INBOX"),
        )
        .await
        .map_err(|_| Error::SelectTimeout {
            mailbox: "INBOX".to_string(),
            timeout: timeouts.select,
        })??;

        debug!("Selected INBOX");

        Ok(session)
    }

    /// Gets the initial UID to start monitoring from.
    async fn get_initial_uid(session: &mut ImapSession, config: &ImapConfig) -> Result<u32> {
        tokio::time::timeout(config.timeouts.uid_fetch, session::get_latest_uid(session))
            .await
            .map_err(|_| Error::UidFetchTimeout {
                timeout: config.timeouts.uid_fetch,
            })?
    }

    /// Calculates the IMAP SINCE date from a `max_age` duration.
    fn calculate_since_date(max_age: Duration) -> NaiveDate {
        let now = Utc::now();
        let since_datetime =
            now - chrono::Duration::from_std(max_age).unwrap_or(chrono::Duration::zero());
        since_datetime.date_naive()
    }

    /// Searches for email UIDs since a given date.
    async fn search_emails_since(&mut self, since_date: NaiveDate) -> Result<Vec<u32>> {
        let timeout = self.config.timeouts.uid_fetch;

        tokio::time::timeout(
            timeout,
            session::search_emails_since(&mut self.session, since_date),
        )
        .await
        .map_err(|_| Error::UidFetchTimeout { timeout })?
    }

    /// Finds matching content in a list of UIDs.
    async fn find_match_in_uids(&mut self, uids: &[u32], matcher: &dyn Matcher) -> Result<String> {
        let fetch_timeout = self.config.timeouts.message_fetch;

        // Search in reverse order (newest first)
        for uid in uids.iter().rev() {
            let uid_str = uid.to_string();

            let mut fetch_result = tokio::time::timeout(
                fetch_timeout,
                session::fetch_messages_by_uid_range(&mut self.session, &uid_str),
            )
            .await
            .map_err(|_| Error::FetchTimeout {
                uid_range: uid_str.clone(),
                timeout: fetch_timeout,
            })??;

            while let Some(message_result) = fetch_result.next().await {
                let message = message_result.map_err(|source| Error::FetchMessage { source })?;

                match parser::extract_match_from_message(&message, matcher) {
                    ExtractResult::Match(result) => return Ok(result.into_owned()),
                    ExtractResult::NoMatch | ExtractResult::ParseError => {
                        // Continue to next message (parse errors are logged in parser)
                    }
                }
            }
        }

        Err(Error::NoMatch)
    }

    /// Checks for new emails and searches for matching content.
    #[instrument(name = "ImapEmailClient::check_new_emails", skip(self, matcher))]
    async fn check_new_emails(&mut self, matcher: &dyn Matcher) -> Result<Option<String>> {
        let timeout = self.config.timeouts.uid_fetch;

        let latest_uid = tokio::time::timeout(timeout, session::get_latest_uid(&mut self.session))
            .await
            .map_err(|_| Error::UidFetchTimeout { timeout })??;

        debug!(
            latest_uid,
            start_uid = self.start_uid,
            "Checking for new emails"
        );

        if latest_uid <= self.start_uid {
            return Ok(None);
        }

        let result = self.search_new_emails(matcher, latest_uid).await?;
        self.start_uid = latest_uid;
        Ok(result)
    }

    /// Searches through new emails for matching pattern.
    #[instrument(
        name = "ImapEmailClient::search_new_emails",
        skip(self, matcher),
        fields(latest_uid)
    )]
    async fn search_new_emails(
        &mut self,
        matcher: &dyn Matcher,
        latest_uid: u32,
    ) -> Result<Option<String>> {
        let fetch_timeout = self.config.timeouts.message_fetch;
        let uid_range = format!("{}:{}", self.start_uid + 1, latest_uid);

        let mut fetch_result = tokio::time::timeout(
            fetch_timeout,
            session::fetch_messages_by_uid_range(&mut self.session, &uid_range),
        )
        .await
        .map_err(|_| Error::FetchTimeout {
            uid_range: uid_range.clone(),
            timeout: fetch_timeout,
        })??;

        while let Some(message_result) = fetch_result.next().await {
            let message = message_result.map_err(|source| Error::FetchMessage { source })?;

            match parser::extract_match_from_message(&message, matcher) {
                ExtractResult::Match(result) => return Ok(Some(result.into_owned())),
                ExtractResult::NoMatch | ExtractResult::ParseError => {
                    // Continue to next message (parse errors are logged in parser)
                }
            }
        }

        Ok(None)
    }
}

impl std::fmt::Debug for ImapEmailClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImapEmailClient")
            .field("email", &self.config.email())
            .field("imap_host", &self.config.effective_imap_host())
            .field("start_uid", &self.start_uid)
            .finish_non_exhaustive()
    }
}

/// RAII guard for [`ImapEmailClient`] that logs out on drop.
///
/// Created by [`ImapEmailClient::into_guard`].
pub struct ImapEmailClientGuard {
    inner: Option<ImapEmailClient>,
}

impl ImapEmailClientGuard {
    /// Waits for an email matching the provided pattern.
    ///
    /// See [`ImapEmailClient::wait_for_match`].
    ///
    /// # Panics
    ///
    /// Panics if the guard has already been consumed (e.g., after calling [`logout`](Self::logout)).
    ///
    /// # Errors
    ///
    /// Returns an error if timeout is reached or IMAP operations fail.
    pub async fn wait_for_match(&mut self, matcher: &dyn Matcher) -> Result<String> {
        self.inner
            .as_mut()
            .expect("guard already consumed")
            .wait_for_match(matcher)
            .await
    }

    /// Finds a matching email among recent messages.
    ///
    /// See [`ImapEmailClient::find_recent_match`].
    ///
    /// # Panics
    ///
    /// Panics if the guard has already been consumed (e.g., after calling [`logout`](Self::logout)).
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoMatch`] if no matching email is found.
    pub async fn find_recent_match(
        &mut self,
        matcher: &dyn Matcher,
        max_age: Duration,
    ) -> Result<String> {
        self.inner
            .as_mut()
            .expect("guard already consumed")
            .find_recent_match(matcher, max_age)
            .await
    }

    /// Explicitly logs out and consumes the guard.
    ///
    /// If not called, the guard will attempt to logout on drop.
    ///
    /// # Errors
    ///
    /// Returns an error if the logout command fails.
    pub async fn logout(mut self) -> Result<()> {
        if let Some(mut client) = self.inner.take() {
            client.logout().await
        } else {
            Ok(())
        }
    }

    /// Returns the email address used for this connection.
    ///
    /// # Panics
    ///
    /// Panics if the guard has already been consumed (e.g., after calling [`logout`](Self::logout)).
    #[must_use]
    pub fn email(&self) -> &str {
        self.inner.as_ref().expect("guard already consumed").email()
    }
}

impl Drop for ImapEmailClientGuard {
    fn drop(&mut self) {
        if let Some(mut client) = self.inner.take() {
            let logout_timeout = client.config.timeouts.logout;

            // Try to get the current tokio runtime handle
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    // We're in an async context, spawn the logout task
                    handle.spawn(async move {
                        match tokio::time::timeout(logout_timeout, client.logout()).await {
                            Ok(Ok(())) => debug!("Client logged out successfully"),
                            Ok(Err(e)) => warn!(error = %e, "Client logout failed"),
                            Err(_) => warn!(
                                timeout_secs = logout_timeout.as_secs(),
                                "Client logout timed out"
                            ),
                        }
                    });
                }
                Err(_) => {
                    // No tokio runtime available - we're in a sync context
                    // Log a warning since we can't perform async logout
                    warn!(
                        "ImapEmailClientGuard dropped outside of tokio runtime context. \
                         Connection will be closed without proper IMAP logout. \
                         Consider calling .logout().await explicitly before dropping."
                    );
                    // The underlying connection will be dropped and closed,
                    // which is not ideal but acceptable as a fallback
                }
            }
        }
    }
}

impl std::fmt::Debug for ImapEmailClientGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImapEmailClientGuard")
            .field("inner", &self.inner)
            .finish()
    }
}
