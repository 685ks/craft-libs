//! The operating system's credential store.

use crate::{SecretError, SecretRef, SecretStore, SecretString};

/// Secrets in the system keychain, filed under one service name.
///
/// Each secret is the entry with this service and the [`SecretRef`] as the account (user)
/// name. Apps that share keys (the launcher and the apps it configures) use the same service;
/// an app keeping private secrets uses its own, such as `photocraft`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeychainStore {
    service: String,
}

impl KeychainStore {
    /// A store for `service`. `craft` is the suite-wide service the launcher writes to.
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }

    /// The service name entries are filed under.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// Whether this build has a system keychain backend at all. Even when true, the store can
    /// still be unavailable at run time (for example with no Secret Service running).
    pub const fn is_supported() -> bool {
        backend::SUPPORTED
    }
}

impl SecretStore for KeychainStore {
    fn get(&self, name: &SecretRef) -> Result<Option<SecretString>, SecretError> {
        backend::get(&self.service, name.as_str())
    }

    fn set(&self, name: &SecretRef, value: &SecretString) -> Result<(), SecretError> {
        backend::set(&self.service, name.as_str(), value)
    }

    fn delete(&self, name: &SecretRef) -> Result<bool, SecretError> {
        backend::delete(&self.service, name.as_str())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
mod backend {
    use super::{SecretError, SecretString};
    use keyring::{Entry, Error};

    pub const SUPPORTED: bool = true;

    pub fn get(service: &str, account: &str) -> Result<Option<SecretString>, SecretError> {
        let entry = Entry::new(service, account).map_err(map)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value))),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(map(e)),
        }
    }

    pub fn set(service: &str, account: &str, value: &SecretString) -> Result<(), SecretError> {
        Entry::new(service, account).and_then(|entry| entry.set_password(value.expose())).map_err(map)
    }

    pub fn delete(service: &str, account: &str) -> Result<bool, SecretError> {
        let entry = Entry::new(service, account).map_err(map)?;
        match entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(Error::NoEntry) => Ok(false),
            Err(e) => Err(map(e)),
        }
    }

    /// Maps a keyring error without ever carrying the secret: `BadEncoding` holds the raw
    /// stored bytes, so only its kind is kept.
    pub(super) fn map(e: Error) -> SecretError {
        map_error(e)
    }

    pub(crate) fn map_error(e: Error) -> SecretError {
        match e {
            Error::NoStorageAccess(inner) => SecretError::Unavailable(inner.to_string()),
            Error::PlatformFailure(inner) => SecretError::Backend(inner.to_string()),
            Error::BadEncoding(_) => SecretError::NotText,
            Error::TooLong(what, limit) => SecretError::TooLong(format!("{what} is limited to {limit}")),
            Error::Invalid(what, why) => SecretError::Backend(format!("{what}: {why}")),
            Error::Ambiguous(_) => SecretError::Backend("more than one keychain entry matches".to_owned()),
            Error::NoEntry => SecretError::Backend("no keychain entry".to_owned()),
            other => SecretError::Backend(other.to_string()),
        }
    }
}

#[cfg(all(
    test,
    any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "openbsd")
))]
pub(crate) fn map_for_test(e: keyring::Error) -> SecretError {
    backend::map_error(e)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "openbsd")))]
mod backend {
    use super::{SecretError, SecretString};

    pub const SUPPORTED: bool = false;

    fn unsupported() -> SecretError {
        SecretError::Unavailable("this platform has no system keychain".to_owned())
    }

    pub fn get(_service: &str, _account: &str) -> Result<Option<SecretString>, SecretError> {
        Err(unsupported())
    }

    pub fn set(_service: &str, _account: &str, _value: &SecretString) -> Result<(), SecretError> {
        Err(unsupported())
    }

    pub fn delete(_service: &str, _account: &str) -> Result<bool, SecretError> {
        Err(unsupported())
    }
}
