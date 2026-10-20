//! Turning a connection's [`SecretRef`] into the key an adapter sends.

use std::fmt;

use craft_ai_core::connection::check_key_header;
use craft_ai_core::{AiError, Auth, Connection, ErrorKind, SecretString};
use craft_secrets::{SecretError, SecretStore};

/// Which header carries the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyHeader {
    /// `Authorization: Bearer <key>`.
    Bearer,
    /// `<name>: <key>`, such as BFL's `x-key`.
    Named(String),
}

/// A resolved key and the header it travels in. `Debug` never shows the key.
#[derive(Clone)]
pub struct ApiKey {
    header: KeyHeader,
    value: SecretString,
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiKey").field("header", &self.header).field("value", &"<redacted>").finish()
    }
}

impl ApiKey {
    /// A key sent as `Authorization: Bearer`.
    pub fn bearer(value: SecretString) -> Self {
        Self { header: KeyHeader::Bearer, value }
    }

    /// A key sent as the whole value of `header`. The name is checked with
    /// [`check_key_header`], so a key can never be sent as `Host` or `Cookie`.
    pub fn in_header(header: &str, value: SecretString) -> Result<Self, AiError> {
        check_key_header(header).map_err(AiError::invalid)?;
        Ok(Self { header: KeyHeader::Named(header.to_owned()), value })
    }

    pub fn header(&self) -> &KeyHeader {
        &self.header
    }

    pub(crate) fn secret(&self) -> &SecretString {
        &self.value
    }

    /// The header name and value to send.
    pub(crate) fn header_pair(&self) -> (String, String) {
        match &self.header {
            KeyHeader::Bearer => ("Authorization".to_owned(), format!("Bearer {}", self.value.expose())),
            KeyHeader::Named(name) => (name.clone(), self.value.expose().to_owned()),
        }
    }
}

/// Reads the key a connection needs from `store`.
///
/// `Ok(None)` for a connection without authentication. A connection that names a key which is
/// not saved, or a keychain that cannot be read, is an `Auth` error the app can act on ("add
/// your key again"). `default_header` is the adapter's own header for providers that do not use
/// bearer tokens; the connection's `key_header` overrides it.
pub fn resolve_key(connection: &Connection, store: &dyn SecretStore, default_header: Option<&str>) -> Result<Option<ApiKey>, AiError> {
    let secret_ref = match &connection.auth {
        Auth::None => return Ok(None),
        Auth::ApiKey { secret } | Auth::Subscription { secret, .. } => secret,
    };
    let value = match store.get(secret_ref) {
        Ok(Some(value)) if !value.is_empty() => value,
        Ok(_) => return Err(AiError::new(ErrorKind::Auth, format!("no key is saved for the connection \"{}\"", connection.label))),
        Err(SecretError::Unavailable(why) | SecretError::Denied(why)) => {
            return Err(AiError::new(ErrorKind::Auth, format!("the key for \"{}\" could not be read: {why}", connection.label)));
        }
        Err(other) => return Err(AiError::new(ErrorKind::Auth, format!("the key for \"{}\" could not be read: {other}", connection.label))),
    };
    match connection.key_header.as_deref().or(default_header) {
        Some(header) => ApiKey::in_header(header, value).map(Some),
        None => Ok(Some(ApiKey::bearer(value))),
    }
}
