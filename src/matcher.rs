//! Email content matching for extracting data from email bodies.
//!
//! This module provides a flexible [`Matcher`] trait and built-in implementations
//! for common patterns like OTP codes and URLs.
//!
//! # Example
//!
//! ```
//! use email_sync::matcher::{RegexMatcher, OtpMatcher, Matcher};
//!
//! // Using built-in OTP matcher
//! let otp = OtpMatcher::six_digit();
//! assert_eq!(otp.find_match("Your code is 123456.").as_deref(), Some("123456"));
//!
//! // Using custom regex
//! let custom = RegexMatcher::new(r"token=([a-f0-9]+)").unwrap();
//! let text = "Click here: https://example.com?token=abc123";
//! assert_eq!(custom.find_match(text).as_deref(), Some("abc123"));
//! ```

use regex::Regex;
use std::borrow::Cow;

/// Trait for matching and extracting content from email bodies.
///
/// Implement this trait to define custom matching logic.
///
/// # Example
///
/// ```
/// use email_sync::matcher::Matcher;
/// use std::borrow::Cow;
///
/// struct JsonFieldMatcher {
///     field: String,
/// }
///
/// impl Matcher for JsonFieldMatcher {
///     fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
///         // Custom JSON parsing logic
///         # None
///     }
///
///     fn description(&self) -> &str {
///         "JSON field extractor"
///     }
/// }
/// ```
pub trait Matcher: Send + Sync {
    /// Attempts to find and extract matching content from the text.
    ///
    /// Returns `Some(matched_value)` if found, `None` otherwise.
    /// Uses `Cow<str>` to avoid allocations when the match can be borrowed
    /// directly from the input text.
    fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>>;

    /// Returns a human-readable description of what this matcher looks for.
    ///
    /// Used in logging and error messages.
    fn description(&self) -> &str;
}

/// Regex-based matcher that extracts the first capture group.
///
/// # Example
///
/// ```
/// use email_sync::matcher::{RegexMatcher, Matcher};
///
/// let matcher = RegexMatcher::new(r"code:\s*(\d+)").unwrap();
/// assert_eq!(matcher.find_match("Your code: 42"), Some("42".into()));
/// ```
#[derive(Debug, Clone)]
pub struct RegexMatcher {
    regex: Regex,
    description: String,
}

impl RegexMatcher {
    /// Creates a new regex matcher.
    ///
    /// The regex should contain at least one capture group. The first capture group
    /// will be extracted as the match result.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::matcher::RegexMatcher;
    ///
    /// let matcher = RegexMatcher::new(r"(\d{6})").unwrap();
    /// ```
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            description: format!("regex pattern: {pattern}"),
            regex,
        })
    }

    /// Creates a new regex matcher with a custom description.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::matcher::RegexMatcher;
    ///
    /// let matcher = RegexMatcher::with_description(
    ///     r"(\d{6})",
    ///     "6-digit verification code"
    /// ).unwrap();
    /// ```
    pub fn with_description(
        pattern: &str,
        description: impl Into<String>,
    ) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            description: description.into(),
            regex,
        })
    }
}

impl Matcher for RegexMatcher {
    fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
        self.regex
            .captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| Cow::Borrowed(m.as_str()))
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Matcher for OTP (One-Time Password) codes.
///
/// # Example
///
/// ```
/// use email_sync::matcher::{OtpMatcher, Matcher};
///
/// let otp = OtpMatcher::six_digit();
/// assert_eq!(otp.find_match("Your code is 123456."), Some("123456".into()));
/// assert_eq!(otp.find_match("Code: 12345"), None); // Only 5 digits
/// ```
#[derive(Debug, Clone)]
pub struct OtpMatcher {
    inner: RegexMatcher,
}

impl OtpMatcher {
    /// Creates a matcher for 6-digit OTP codes.
    ///
    /// Matches exactly 6 consecutive digits that appear after non-digit characters
    /// (or start of string) and are followed by a period or non-digit.
    #[must_use]
    pub fn six_digit() -> Self {
        Self::n_digit(6)
    }

    /// Creates a matcher for N-digit OTP codes.
    ///
    /// Uses word boundaries to match exactly N digits.
    ///
    /// # Panics
    ///
    /// Panics if `digits` is 0.
    #[must_use]
    pub fn n_digit(digits: usize) -> Self {
        assert!(digits > 0, "digits must be > 0");
        // Use \b word boundary - matches between a word char and non-word char
        // This ensures we don't match partial digit sequences
        let pattern = format!(r"\b(\d{{{digits}}})\b");
        Self {
            inner: RegexMatcher::with_description(&pattern, format!("{digits}-digit OTP code"))
                .expect("valid regex"),
        }
    }

    /// Creates a matcher for OTP codes with custom regex.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn custom(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            inner: RegexMatcher::with_description(pattern, "custom OTP pattern")?,
        })
    }
}

impl Matcher for OtpMatcher {
    fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
        self.inner.find_match(text)
    }

    fn description(&self) -> &str {
        self.inner.description()
    }
}

/// Matcher for URLs matching a specific domain pattern.
///
/// # Example
///
/// ```
/// use email_sync::matcher::{UrlMatcher, Matcher};
///
/// let matcher = UrlMatcher::new("example.com");
/// let text = r#"<a href="https://example.com/verify?token=abc">Click</a>"#;
/// assert_eq!(matcher.find_match(text), Some("https://example.com/verify?token=abc".into()));
/// ```
#[derive(Debug, Clone)]
pub struct UrlMatcher {
    inner: RegexMatcher,
}

impl UrlMatcher {
    /// Creates a matcher for URLs containing the specified domain.
    ///
    /// # Panics
    ///
    /// Panics if the regex pattern cannot be compiled (should not happen with valid domain).
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::matcher::UrlMatcher;
    ///
    /// let matcher = UrlMatcher::new("example.com");
    /// ```
    #[must_use]
    pub fn new(domain: &str) -> Self {
        // Escape dots in domain for regex
        let escaped_domain = domain.replace('.', r"\.");
        let pattern = format!(r#"href="(https?://{escaped_domain}[^"]*)""#);
        Self {
            inner: RegexMatcher::with_description(&pattern, format!("URL from {domain}"))
                .expect("valid regex"),
        }
    }

    /// Creates a matcher with a custom URL regex pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn custom(pattern: &str, description: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            inner: RegexMatcher::with_description(pattern, description)?,
        })
    }
}

impl Matcher for UrlMatcher {
    fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
        self.inner.find_match(text)
    }

    fn description(&self) -> &str {
        self.inner.description()
    }
}

/// Matcher using a closure for custom matching logic.
///
/// # Example
///
/// ```
/// use email_sync::matcher::{ClosureMatcher, Matcher};
/// use std::borrow::Cow;
///
/// let matcher = ClosureMatcher::new(
///     |text| {
///         text.lines()
///             .find(|line| line.starts_with("Code:"))
///             .map(|line| Cow::Owned(line.trim_start_matches("Code:").trim().to_string()))
///     },
///     "code line extractor"
/// );
///
/// let text = "Hello\nCode: ABC123\nThanks";
/// assert_eq!(matcher.find_match(text).as_deref(), Some("ABC123"));
/// ```
pub struct ClosureMatcher<F>
where
    F: for<'a> Fn(&'a str) -> Option<Cow<'a, str>> + Send + Sync,
{
    matcher_fn: F,
    description: String,
}

impl<F> ClosureMatcher<F>
where
    F: for<'a> Fn(&'a str) -> Option<Cow<'a, str>> + Send + Sync,
{
    /// Creates a new closure-based matcher.
    #[must_use]
    pub fn new(matcher_fn: F, description: impl Into<String>) -> Self {
        Self {
            matcher_fn,
            description: description.into(),
        }
    }
}

impl<F> Matcher for ClosureMatcher<F>
where
    F: for<'a> Fn(&'a str) -> Option<Cow<'a, str>> + Send + Sync,
{
    fn find_match<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
        (self.matcher_fn)(text)
    }

    fn description(&self) -> &str {
        &self.description
    }
}

impl<F> std::fmt::Debug for ClosureMatcher<F>
where
    F: for<'a> Fn(&'a str) -> Option<Cow<'a, str>> + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosureMatcher")
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_matcher() {
        let matcher = RegexMatcher::new(r"code:\s*(\d+)").unwrap();
        assert_eq!(
            matcher.find_match("Your code: 12345").as_deref(),
            Some("12345")
        );
        assert_eq!(matcher.find_match("No code here"), None);
    }

    #[test]
    fn test_otp_six_digit() {
        let otp = OtpMatcher::six_digit();
        assert_eq!(
            otp.find_match("Your code is 123456.").as_deref(),
            Some("123456")
        );
        assert_eq!(
            otp.find_match("Your code is 123456").as_deref(),
            Some("123456")
        ); // No period
        assert_eq!(otp.find_match("Code: 12345"), None); // Only 5 digits
        assert_eq!(otp.find_match("Code: 1234567"), None); // 7 digits
    }

    #[test]
    fn test_otp_n_digit() {
        let otp = OtpMatcher::n_digit(4);
        assert_eq!(otp.find_match("PIN: 1234").as_deref(), Some("1234"));
        assert_eq!(otp.find_match("PIN: 12345"), None); // 5 digits
    }

    #[test]
    fn test_url_matcher() {
        let matcher = UrlMatcher::new("example.com");
        let html = r#"<a href="https://example.com/verify?token=abc123">Click here</a>"#;
        assert_eq!(
            matcher.find_match(html).as_deref(),
            Some("https://example.com/verify?token=abc123")
        );
    }

    #[test]
    fn test_url_matcher_no_match() {
        let matcher = UrlMatcher::new("example.com");
        let html = r#"<a href="https://other.com/page">Click here</a>"#;
        assert_eq!(matcher.find_match(html), None);
    }

    #[test]
    fn test_closure_matcher() {
        let matcher = ClosureMatcher::new(
            |text| {
                text.lines()
                    .find(|line| line.contains("SECRET"))
                    .map(|line| Cow::Owned(line.replace("SECRET:", "").trim().to_string()))
            },
            "secret extractor",
        );

        let text = "Header\nSECRET: my-value\nFooter";
        assert_eq!(matcher.find_match(text).as_deref(), Some("my-value"));
    }

    #[test]
    fn test_example_activation_pattern() {
        let matcher = UrlMatcher::new("example.com");
        let html = r#"<a href="https://example.com/activate?token=abc123">Activate</a>"#;
        assert_eq!(
            matcher.find_match(html).as_deref(),
            Some("https://example.com/activate?token=abc123")
        );
    }

    #[test]
    fn test_regex_matcher_returns_borrowed() {
        // Verify that RegexMatcher returns a borrowed reference (no allocation)
        let matcher = RegexMatcher::new(r"code:\s*(\d+)").unwrap();
        let result = matcher.find_match("Your code: 12345");
        assert!(matches!(result, Some(Cow::Borrowed(_))));
    }
}
