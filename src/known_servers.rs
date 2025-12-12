//! IMAP server discovery from email domains.
//!
//! This module provides automatic IMAP server hostname discovery for common
//! email providers, with support for runtime customization.
//!
//! # Example
//!
//! ```
//! use email_sync::known_servers::{ServerRegistry, discover_imap_host};
//!
//! // Use built-in discovery
//! assert_eq!(discover_imap_host("user@gmail.com"), "imap.gmail.com");
//!
//! // Create a custom registry for your application
//! let mut registry = ServerRegistry::with_defaults();
//! registry.register("mycompany.com", "mail.mycompany.com");
//! assert_eq!(registry.discover("user@mycompany.com"), "mail.mycompany.com");
//! ```

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Map of email domains to their IMAP server hostnames.
static KNOWN_SERVERS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Google
    m.insert("gmail.com", "imap.gmail.com");

    // Yahoo
    m.insert("yahoo.com", "imap.mail.yahoo.com");

    // Microsoft
    m.insert("hotmail.com", "imap-mail.outlook.com");
    m.insert("outlook.com", "imap-mail.outlook.com");
    m.insert("live.com", "imap-mail.outlook.com");

    // Mail.ru network
    m.insert("mail.ru", "imap.mail.ru");
    m.insert("internet.ru", "imap.mail.ru");
    m.insert("bk.ru", "imap.mail.ru");
    m.insert("inbox.ru", "imap.mail.ru");
    m.insert("list.ru", "imap.mail.ru");

    // AOL
    m.insert("aol.com", "imap.aol.com");

    // Yandex
    m.insert("yandex.ru", "imap.yandex.ru");
    m.insert("yandex.com", "imap.yandex.ru");

    // Apple
    m.insert("icloud.com", "imap.mail.me.com");
    m.insert("me.com", "imap.mail.me.com");
    m.insert("mac.com", "imap.mail.me.com");

    // German providers
    m.insert("web.de", "imap.web.de");
    m.insert("gmx.de", "imap.gmx.net");
    m.insert("gmx.at", "imap.gmx.net");
    m.insert("gmx.ch", "imap.gmx.net");
    m.insert("gmx.net", "imap.gmx.net");
    m.insert("gmx.com", "imap.gmx.net");
    m.insert("t-online.de", "secureimap.t-online.de");
    m.insert("firemail.de", "imap.firemail.de");

    // Polish providers
    m.insert("gazeta.pl", "imap.gazeta.pl");

    // Russian providers
    m.insert("rambler.ru", "imap.rambler.ru");

    // FirstMail network
    m.insert("streetwormail.com", "imap.firstmail.ltd");
    m.insert("bonsoirmail.com", "imap.firstmail.ltd");
    m.insert("aurevoirmail.com", "imap.firstmail.ltd");
    m.insert("bonjourfmail.com", "imap.firstmail.ltd");
    m.insert("bientotmail.com", "imap.firstmail.ltd");

    m
});

/// A customizable registry for IMAP server discovery.
///
/// This allows you to add custom domain-to-IMAP-host mappings at runtime,
/// in addition to (or overriding) the built-in defaults.
///
/// # Example
///
/// ```
/// use email_sync::known_servers::ServerRegistry;
///
/// // Start with defaults and add custom mappings
/// let mut registry = ServerRegistry::with_defaults();
/// registry.register("mycompany.com", "imap.mycompany.internal");
/// registry.register("partner.org", "mail.partner.org");
///
/// assert_eq!(registry.discover("user@mycompany.com"), "imap.mycompany.internal");
/// assert_eq!(registry.discover("user@gmail.com"), "imap.gmail.com"); // Built-in
/// ```
#[derive(Debug, Clone)]
pub struct ServerRegistry {
    custom: HashMap<String, String>,
    use_defaults: bool,
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerRegistry {
    /// Creates an empty registry without built-in defaults.
    ///
    /// Use [`Self::with_defaults`] if you want to include the standard mappings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            custom: HashMap::new(),
            use_defaults: false,
        }
    }

    /// Creates a registry that includes built-in default mappings.
    ///
    /// Custom mappings added via [`Self::register`] will override defaults.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            custom: HashMap::new(),
            use_defaults: true,
        }
    }

    /// Registers a custom domain-to-IMAP-host mapping.
    ///
    /// This will override any existing mapping (including built-in defaults).
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::known_servers::ServerRegistry;
    ///
    /// let mut registry = ServerRegistry::with_defaults();
    /// registry.register("custom.org", "imap.custom.org");
    /// ```
    pub fn register(&mut self, domain: impl Into<String>, imap_host: impl Into<String>) {
        self.custom
            .insert(domain.into().to_lowercase(), imap_host.into());
    }

    /// Registers multiple domain mappings at once.
    ///
    /// # Example
    ///
    /// ```
    /// use email_sync::known_servers::ServerRegistry;
    ///
    /// let mut registry = ServerRegistry::with_defaults();
    /// registry.register_many([
    ///     ("corp.com", "mail.corp.com"),
    ///     ("partner.org", "imap.partner.org"),
    /// ]);
    /// ```
    pub fn register_many<I, D, H>(&mut self, mappings: I)
    where
        I: IntoIterator<Item = (D, H)>,
        D: Into<String>,
        H: Into<String>,
    {
        for (domain, host) in mappings {
            self.register(domain, host);
        }
    }

    /// Removes a custom mapping.
    ///
    /// Note: This only removes custom mappings, not built-in defaults.
    /// To completely disable a domain, register it with an empty string
    /// or use a registry without defaults.
    pub fn unregister(&mut self, domain: &str) -> Option<String> {
        self.custom.remove(&domain.to_lowercase())
    }

    /// Discovers the IMAP hostname for an email address.
    ///
    /// Resolution order:
    /// 1. Custom mappings (added via [`Self::register`])
    /// 2. Built-in defaults (if [`Self::with_defaults`] was used)
    /// 3. Fallback to `imap.{domain}`
    #[must_use]
    pub fn discover(&self, email: &str) -> Cow<'_, str> {
        let domain = email.split('@').nth(1).unwrap_or(email).to_lowercase();

        // Check custom mappings first
        if let Some(host) = self.custom.get(&domain) {
            return Cow::Borrowed(host);
        }

        // Check built-in defaults
        if self.use_defaults {
            if let Some(&host) = KNOWN_SERVERS.get(domain.as_str()) {
                return Cow::Borrowed(host);
            }
        }

        // Fallback
        Cow::Owned(format!("imap.{domain}"))
    }

    /// Returns `true` if the domain has a known IMAP server mapping.
    #[must_use]
    pub fn is_known(&self, domain: &str) -> bool {
        let domain_lower = domain.to_lowercase();
        self.custom.contains_key(&domain_lower)
            || (self.use_defaults && KNOWN_SERVERS.contains_key(domain_lower.as_str()))
    }

    /// Returns all registered domains (custom + defaults if enabled).
    #[must_use]
    pub fn domains(&self) -> Vec<Cow<'_, str>> {
        let mut domains: Vec<Cow<'_, str>> = self
            .custom
            .keys()
            .map(|s| Cow::Borrowed(s.as_str()))
            .collect();

        if self.use_defaults {
            for &domain in KNOWN_SERVERS.keys() {
                if !self.custom.contains_key(domain) {
                    domains.push(Cow::Borrowed(domain));
                }
            }
        }

        domains
    }

    /// Returns the number of registered mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        let default_count = if self.use_defaults {
            KNOWN_SERVERS
                .keys()
                .filter(|k| !self.custom.contains_key(**k))
                .count()
        } else {
            0
        };
        self.custom.len() + default_count
    }

    /// Returns `true` if the registry has no mappings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.custom.is_empty() && (!self.use_defaults || KNOWN_SERVERS.is_empty())
    }
}

/// Discovers the IMAP hostname for an email address.
///
/// If the domain is known, returns the corresponding IMAP server.
/// Otherwise, returns a default of `imap.{domain}`.
///
/// # Example
///
/// ```
/// use email_sync::known_servers::discover_imap_host;
///
/// assert_eq!(discover_imap_host("user@gmail.com"), "imap.gmail.com");
/// assert_eq!(discover_imap_host("user@custom.org"), "imap.custom.org");
/// ```
#[must_use]
pub fn discover_imap_host(email: &str) -> String {
    let domain = email.split('@').nth(1).unwrap_or(email).to_lowercase();

    KNOWN_SERVERS
        .get(domain.as_str())
        .map_or_else(|| format!("imap.{domain}"), |&s| s.to_string())
}

/// Returns `true` if the domain has a known IMAP server mapping.
#[must_use]
pub fn is_known_domain(domain: &str) -> bool {
    KNOWN_SERVERS.contains_key(domain.to_lowercase().as_str())
}

/// Returns all known email domains.
#[must_use]
pub fn known_domains() -> Vec<&'static str> {
    KNOWN_SERVERS.keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gmail() {
        assert_eq!(discover_imap_host("user@gmail.com"), "imap.gmail.com");
    }

    #[test]
    fn test_outlook() {
        assert_eq!(
            discover_imap_host("user@outlook.com"),
            "imap-mail.outlook.com"
        );
        assert_eq!(
            discover_imap_host("user@hotmail.com"),
            "imap-mail.outlook.com"
        );
    }

    #[test]
    fn test_mail_ru_network() {
        assert_eq!(discover_imap_host("user@mail.ru"), "imap.mail.ru");
        assert_eq!(discover_imap_host("user@bk.ru"), "imap.mail.ru");
        assert_eq!(discover_imap_host("user@inbox.ru"), "imap.mail.ru");
    }

    #[test]
    fn test_unknown_domain() {
        assert_eq!(discover_imap_host("user@example.com"), "imap.example.com");
        assert_eq!(
            discover_imap_host("user@mycompany.org"),
            "imap.mycompany.org"
        );
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(discover_imap_host("user@GMAIL.COM"), "imap.gmail.com");
        assert_eq!(discover_imap_host("user@Gmail.Com"), "imap.gmail.com");
    }

    #[test]
    fn test_is_known_domain() {
        assert!(is_known_domain("gmail.com"));
        assert!(is_known_domain("outlook.com"));
        assert!(!is_known_domain("example.com"));
    }

    #[test]
    fn test_known_domains_not_empty() {
        assert!(!known_domains().is_empty());
        assert!(known_domains().contains(&"gmail.com"));
    }

    // ServerRegistry tests

    #[test]
    fn test_registry_empty() {
        let registry = ServerRegistry::new();
        assert!(!registry.is_known("gmail.com"));
        assert_eq!(
            registry.discover("user@gmail.com").as_ref(),
            "imap.gmail.com"
        );
    }

    #[test]
    fn test_registry_with_defaults() {
        let registry = ServerRegistry::with_defaults();
        assert!(registry.is_known("gmail.com"));
        assert_eq!(
            registry.discover("user@gmail.com").as_ref(),
            "imap.gmail.com"
        );
    }

    #[test]
    fn test_registry_custom_mapping() {
        let mut registry = ServerRegistry::new();
        registry.register("mycompany.com", "mail.internal.mycompany.com");

        assert!(registry.is_known("mycompany.com"));
        assert_eq!(
            registry.discover("user@mycompany.com").as_ref(),
            "mail.internal.mycompany.com"
        );
    }

    #[test]
    fn test_registry_override_default() {
        let mut registry = ServerRegistry::with_defaults();
        registry.register("gmail.com", "custom-gmail.example.com");

        assert_eq!(
            registry.discover("user@gmail.com").as_ref(),
            "custom-gmail.example.com"
        );
    }

    #[test]
    fn test_registry_register_many() {
        let mut registry = ServerRegistry::new();
        registry.register_many([
            ("corp.com", "mail.corp.com"),
            ("partner.org", "imap.partner.org"),
        ]);

        assert_eq!(registry.discover("user@corp.com").as_ref(), "mail.corp.com");
        assert_eq!(
            registry.discover("user@partner.org").as_ref(),
            "imap.partner.org"
        );
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = ServerRegistry::new();
        registry.register("test.com", "mail.test.com");
        assert!(registry.is_known("test.com"));

        registry.unregister("test.com");
        assert!(!registry.is_known("test.com"));
    }

    #[test]
    fn test_registry_case_insensitive() {
        let mut registry = ServerRegistry::new();
        registry.register("MyCompany.COM", "mail.mycompany.com");

        assert!(registry.is_known("mycompany.com"));
        assert!(registry.is_known("MYCOMPANY.COM"));
        assert_eq!(
            registry.discover("user@MYCOMPANY.COM").as_ref(),
            "mail.mycompany.com"
        );
    }

    #[test]
    fn test_registry_len() {
        let mut registry = ServerRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        registry.register("test.com", "mail.test.com");
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let registry_with_defaults = ServerRegistry::with_defaults();
        assert!(!registry_with_defaults.is_empty());
    }
}
