//! Internal IMAP session management.
//!
//! This module wraps async-imap operations with proper error handling.

use crate::connection::TlsStream;
use crate::error::{Error, Result};
use async_imap::Session;
use chrono::NaiveDate;
use futures::stream::BoxStream;
use futures::StreamExt;
use tracing::{debug, instrument};

/// Type alias for IMAP session over TLS.
pub(crate) type ImapSession = Session<TlsStream>;

/// Authentication configuration for IMAP.
pub(crate) struct AuthConfig<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

/// Authenticates to IMAP server and returns a session.
#[instrument(
    name = "authenticate",
    target = "email.session",
    skip_all,
    fields(email = %config.email)
)]
pub(crate) async fn authenticate(
    tls_stream: TlsStream,
    config: &AuthConfig<'_>,
) -> Result<ImapSession> {
    let client = async_imap::Client::new(tls_stream);

    let result = async {
        debug!(target: "email.session", "Authenticating to IMAP server");

        client
            .login(config.email, config.password)
            .await
            .map_err(|e| Error::ImapLogin {
                email: config.email.to_string(),
                source: e.0,
            })
    }
    .await;

    crate::otel::set_span_status(&result);
    result
}

/// Selects a mailbox (typically "INBOX").
#[instrument(name = "select", target = "email.session", skip(session), fields(mailbox = %mailbox))]
pub(crate) async fn select_mailbox(session: &mut ImapSession, mailbox: &str) -> Result<()> {
    let result = async {
        debug!(target: "email.session", "Selecting mailbox");

        session
            .select(mailbox)
            .await
            .map_err(|source| Error::SelectMailbox {
                mailbox: mailbox.to_string(),
                source,
            })?;

        Ok(())
    }
    .await;

    crate::otel::set_span_status(&result);
    result
}

/// Gets the latest UID from the current mailbox.
#[instrument(
    name = "get_latest_uid",
    target = "email.session",
    skip(session),
    fields(max_uid, uid_count)
)]
pub(crate) async fn get_latest_uid(session: &mut ImapSession) -> Result<u32> {
    let result = async {
        // NOOP to ensure we have latest state
        session
            .noop()
            .await
            .map_err(|source| Error::ImapNoop { source })?;

        let uids = session
            .uid_search("ALL")
            .await
            .map_err(|source| Error::ImapSearch { source })?;

        let max_uid = uids.iter().max().copied().unwrap_or(0);
        let span = tracing::Span::current();
        let uid_count = uids.len() as u64;

        span.record("max_uid", max_uid);
        span.record("uid_count", uid_count);

        debug!(target: "email.session", max_uid, uid_count, "Retrieved latest UID");

        Ok(max_uid)
    }
    .await;

    crate::otel::set_span_status(&result);
    result
}

/// Searches for email UIDs since a given date.
#[instrument(
    name = "search_since",
    target = "email.session",
    skip(session),
    fields(since_date = %since_date, uid_count)
)]
pub(crate) async fn search_emails_since(
    session: &mut ImapSession,
    since_date: NaiveDate,
) -> Result<Vec<u32>> {
    let result = async {
        // NOOP to ensure we have latest state
        session
            .noop()
            .await
            .map_err(|source| Error::ImapNoop { source })?;

        // IMAP SINCE format: "DD-Mon-YYYY" (e.g., "07-Dec-YYYY")
        let since_str = since_date.format("%d-%b-%Y").to_string();
        let query = format!("SINCE {since_str}");

        let uids = session
            .uid_search(&query)
            .await
            .map_err(|source| Error::ImapSearch { source })?;

        let uids_vec: Vec<u32> = uids.into_iter().collect();
        let uid_count = uids_vec.len() as u64;

        tracing::Span::current().record("uid_count", uid_count);

        debug!(
            target: "email.session",
            uid_count,
            since = %since_str,
            "Found emails"
        );

        Ok(uids_vec)
    }
    .await;

    crate::otel::set_span_status(&result);
    result
}

/// Fetches messages by UID range.
///
/// Returns a boxed stream of fetch results.
pub(crate) async fn fetch_messages_by_uid_range<'a>(
    session: &'a mut ImapSession,
    uid_range: &str,
) -> Result<BoxStream<'a, std::result::Result<async_imap::types::Fetch, async_imap::error::Error>>>
{
    debug!(target: "email.session", uid_range = %uid_range, "Fetching messages");

    let stream = session
        .uid_fetch(uid_range, "BODY[]")
        .await
        .map_err(|source| Error::ImapFetch {
            uid_range: uid_range.to_string(),
            source,
        })?;

    Ok(stream.boxed())
}

/// Logs out from IMAP session.
#[instrument(name = "logout", target = "email.session", skip(session))]
pub(crate) async fn logout(session: &mut ImapSession) -> Result<()> {
    let result = async {
        debug!(target: "email.session", "Logging out");

        session
            .logout()
            .await
            .map_err(|source| Error::ImapLogout { source })?;

        Ok(())
    }
    .await;

    crate::otel::set_span_status(&result);
    result
}
