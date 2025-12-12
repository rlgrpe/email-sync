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
    name = "session::authenticate",
    skip_all,
    fields(email = %config.email)
)]
pub(crate) async fn authenticate(
    tls_stream: TlsStream,
    config: &AuthConfig<'_>,
) -> Result<ImapSession> {
    let client = async_imap::Client::new(tls_stream);

    debug!("Authenticating to IMAP server");

    client
        .login(config.email, config.password)
        .await
        .map_err(|e| Error::ImapLogin {
            email: config.email.to_string(),
            source: e.0,
        })
}

/// Selects a mailbox (typically "INBOX").
#[instrument(name = "session::select", skip(session), fields(mailbox = %mailbox))]
pub(crate) async fn select_mailbox(session: &mut ImapSession, mailbox: &str) -> Result<()> {
    debug!("Selecting mailbox");

    session
        .select(mailbox)
        .await
        .map_err(|source| Error::SelectMailbox {
            mailbox: mailbox.to_string(),
            source,
        })?;

    Ok(())
}

/// Gets the latest UID from the current mailbox.
#[instrument(name = "session::get_latest_uid", skip(session))]
pub(crate) async fn get_latest_uid(session: &mut ImapSession) -> Result<u32> {
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

    debug!(max_uid, uid_count = uids.len(), "Retrieved latest UID");

    Ok(max_uid)
}

/// Searches for email UIDs since a given date.
#[instrument(
    name = "session::search_since",
    skip(session),
    fields(since_date = %since_date)
)]
pub(crate) async fn search_emails_since(
    session: &mut ImapSession,
    since_date: NaiveDate,
) -> Result<Vec<u32>> {
    // NOOP to ensure we have latest state
    session
        .noop()
        .await
        .map_err(|source| Error::ImapNoop { source })?;

    // IMAP SINCE format: "DD-Mon-YYYY" (e.g., "07-Dec-2025")
    let since_str = since_date.format("%d-%b-%Y").to_string();
    let query = format!("SINCE {since_str}");

    let uids = session
        .uid_search(&query)
        .await
        .map_err(|source| Error::ImapSearch { source })?;

    let uids_vec: Vec<u32> = uids.into_iter().collect();

    debug!(
        uid_count = uids_vec.len(),
        since = %since_str,
        "Found emails"
    );

    Ok(uids_vec)
}

/// Fetches messages by UID range.
///
/// Returns a boxed stream of fetch results.
pub(crate) async fn fetch_messages_by_uid_range<'a>(
    session: &'a mut ImapSession,
    uid_range: &str,
) -> Result<BoxStream<'a, std::result::Result<async_imap::types::Fetch, async_imap::error::Error>>>
{
    debug!(uid_range = %uid_range, "Fetching messages");

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
#[instrument(name = "session::logout", skip(session))]
pub(crate) async fn logout(session: &mut ImapSession) -> Result<()> {
    debug!("Logging out");

    session
        .logout()
        .await
        .map_err(|source| Error::ImapLogout { source })?;

    Ok(())
}
