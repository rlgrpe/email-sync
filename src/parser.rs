//! Internal module for parsing email content.

use crate::matcher::Matcher;
use mailparse::parse_mail;
use std::borrow::Cow;
use tracing::{debug, warn};

/// Result of attempting to extract a match from a message.
#[derive(Debug)]
pub(crate) enum ExtractResult<'a> {
    /// A match was found
    Match(Cow<'a, str>),
    /// No match in this message
    NoMatch,
    /// Message couldn't be parsed (logged, but can continue to next message)
    ParseError,
}

/// Extracts matching content from an IMAP fetch result using the provided matcher.
///
/// This function is designed to be resilient - it will log and skip malformed messages
/// rather than failing the entire operation. This allows processing to continue even
/// if some emails have parsing issues.
pub(crate) fn extract_match_from_message(
    message: &async_imap::types::Fetch,
    pattern_matcher: &dyn Matcher,
) -> ExtractResult<'static> {
    let uid = message.uid;

    let Some(body) = message.body() else {
        debug!(uid, "Message has no body");
        return ExtractResult::NoMatch;
    };

    let parsed = match parse_mail(body) {
        Ok(p) => p,
        Err(e) => {
            warn!(
                uid,
                error = %e,
                "Failed to parse email, skipping message"
            );
            return ExtractResult::ParseError;
        }
    };

    // Try to get the body, handling multipart messages
    let text = match extract_body_text(&parsed) {
        Ok(t) => t,
        Err(e) => {
            warn!(
                uid,
                error = %e,
                "Failed to extract body from email, skipping message"
            );
            return ExtractResult::ParseError;
        }
    };

    if let Some(result) = pattern_matcher.find_match(&text) {
        debug!(
            uid,
            matcher = %pattern_matcher.description(),
            matched_len = result.len(),
            "Found match in email"
        );
        // Convert the Cow result to an owned Cow since we can't keep
        // borrowing from `text` (a local variable)
        ExtractResult::Match(Cow::Owned(result.into_owned()))
    } else {
        debug!(
            uid,
            matcher = %pattern_matcher.description(),
            "No match found in email body"
        );
        ExtractResult::NoMatch
    }
}

/// Extracts text content from a parsed email, handling multipart messages.
fn extract_body_text(
    parsed: &mailparse::ParsedMail<'_>,
) -> Result<String, mailparse::MailParseError> {
    // If the message has subparts, try to find text content
    if !parsed.subparts.is_empty() {
        // Look for text/plain first, then text/html
        for part in &parsed.subparts {
            let content_type = part.ctype.mimetype.to_lowercase();
            if content_type == "text/plain" || content_type == "text/html" {
                if let Ok(body) = part.get_body() {
                    return Ok(body);
                }
            }
        }

        // If no text parts found, try to get body from first subpart
        if let Some(first_part) = parsed.subparts.first() {
            return extract_body_text(first_part);
        }
    }

    // Single part message or fallback
    parsed.get_body()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::OtpMatcher;

    #[test]
    fn test_extract_body_text_simple() {
        let raw = b"From: test@example.com\r\nTo: user@example.com\r\n\r\nYour code is 123456.";
        let parsed = parse_mail(raw).unwrap();
        let text = extract_body_text(&parsed).unwrap();
        assert!(text.contains("123456"));
    }

    #[test]
    fn test_matcher_integration() {
        let raw = b"From: test@example.com\r\nTo: user@example.com\r\n\r\nYour verification code is 654321.";
        let parsed = parse_mail(raw).unwrap();
        let text = extract_body_text(&parsed).unwrap();

        let matcher = OtpMatcher::six_digit();
        let result = matcher.find_match(&text);
        assert_eq!(result.as_deref(), Some("654321"));
    }

    #[test]
    fn test_extract_result_variants() {
        // Test that ExtractResult has the expected variants
        let match_result: ExtractResult<'_> = ExtractResult::Match(Cow::Borrowed("test"));
        assert!(matches!(match_result, ExtractResult::Match(_)));

        let no_match: ExtractResult<'_> = ExtractResult::NoMatch;
        assert!(matches!(no_match, ExtractResult::NoMatch));

        let parse_error: ExtractResult<'_> = ExtractResult::ParseError;
        assert!(matches!(parse_error, ExtractResult::ParseError));
    }
}
