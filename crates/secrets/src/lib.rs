#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! Secret storage behind [`SecretRef`].
//!
//! The Craft data model never holds a secret: a connection stores a [`SecretRef`], a name for
//! the secret, and the value lives in the operating system's credential store. This crate is
//! that store:
//!
//! - [`KeychainStore`] uses the macOS Keychain, the Windows Credential Manager, or the Secret
//!   Service (GNOME Keyring, KWallet) on Linux and the BSDs, through the `keyring` crate. On
//!   other targets (such as the web) every call reports [`SecretError::Unavailable`].
//! - [`MemoryStore`] keeps secrets in memory, for tests and for a "don't remember" session.
//!
//! Values go in and come out as [`SecretString`], which cannot be printed or serialized by
//! accident, and no error message ever contains a secret value.
//!
//! ```no_run
//! use craft_secrets::{KeychainStore, SecretStore};
//! use craft_ai_core::{SecretRef, SecretString};
//!
//! let store = KeychainStore::new("craft");
//! let key = SecretRef::new("openai.key").map_err(|_| "bad ref")?;
//! store.set(&key, &SecretString::new("sk-…"))?;
//! if let Some(value) = store.get(&key)? {
//!     // use value.expose() in the outgoing request only
//! #   let _ = value;
//! }
//! store.delete(&key)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

pub use craft_ai_core::draft::KeyChange;
pub use craft_ai_core::{SecretRef, SecretString};

mod keychain;
pub use keychain::KeychainStore;

#[cfg(test)]
mod tests;

/// Why a secret could not be read, written or removed. Never contains the secret itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretError {
    /// The credential store cannot be used: the platform has none, or the keychain is locked
    /// and cannot ask the user. Offer to keep the secret for this session only (a
    /// [`MemoryStore`]) or to enter it each time.
    Unavailable(String),
    /// The store refused access (the user denied the prompt, or a policy forbids it).
    Denied(String),
    /// The stored value is not valid UTF-8 text, so it cannot be a key this suite wrote.
    NotText,
    /// The value or name is longer than the platform store accepts.
    TooLong(String),
    /// Any other failure reported by the store, including a Linux session with no Secret
    /// Service running. Treat it like [`SecretError::Unavailable`] in the UI.
    Backend(String),
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "the system keychain is not available: {why}"),
            Self::Denied(why) => write!(f, "access to the system keychain was refused: {why}"),
            Self::NotText => f.write_str("the saved secret is not text"),
            Self::TooLong(why) => write!(f, "the secret is too long for the system keychain: {why}"),
            Self::Backend(why) => write!(f, "the system keychain reported an error: {why}"),
        }
    }
}

impl std::error::Error for SecretError {}

/// A place that keeps secret values under [`SecretRef`] names.
pub trait SecretStore: Send + Sync {
    /// The secret named `name`, or `None` when nothing is saved under it.
    fn get(&self, name: &SecretRef) -> Result<Option<SecretString>, SecretError>;

    /// Saves `value` under `name`, replacing what was there.
    fn set(&self, name: &SecretRef, value: &SecretString) -> Result<(), SecretError>;

    /// Removes the secret named `name`. Returns whether there was one.
    fn delete(&self, name: &SecretRef) -> Result<bool, SecretError>;

    /// Carries out a connection form's [`KeyChange`] for the secret named `name`.
    fn apply(&self, name: &SecretRef, change: &KeyChange) -> Result<(), SecretError> {
        match change {
            KeyChange::Keep => Ok(()),
            KeyChange::Set(value) => self.set(name, value),
            KeyChange::Clear => self.delete(name).map(|_| ()),
        }
    }
}

impl<T: SecretStore + ?Sized> SecretStore for Arc<T> {
    fn get(&self, name: &SecretRef) -> Result<Option<SecretString>, SecretError> {
        (**self).get(name)
    }

    fn set(&self, name: &SecretRef, value: &SecretString) -> Result<(), SecretError> {
        (**self).set(name, value)
    }

    fn delete(&self, name: &SecretRef) -> Result<bool, SecretError> {
        (**self).delete(name)
    }
}

impl<T: SecretStore + ?Sized> SecretStore for Box<T> {
    fn get(&self, name: &SecretRef) -> Result<Option<SecretString>, SecretError> {
        (**self).get(name)
    }

    fn set(&self, name: &SecretRef, value: &SecretString) -> Result<(), SecretError> {
        (**self).set(name, value)
    }

    fn delete(&self, name: &SecretRef) -> Result<bool, SecretError> {
        (**self).delete(name)
    }
}

/// Secrets kept in memory only, gone when the process ends.
///
/// For tests, and for a session where the user chose not to save a key or the keychain is
/// unavailable. Clones share the same secrets.
#[derive(Clone, Default)]
pub struct MemoryStore {
    secrets: Arc<Mutex<HashMap<SecretRef, SecretString>>>,
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStore").field("len", &self.len()).finish_non_exhaustive()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many secrets are held.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forgets every secret.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// A poisoned lock only means another thread panicked while holding it; the map is still
    /// whole, so keep using it rather than failing.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SecretRef, SecretString>> {
        self.secrets.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &SecretRef) -> Result<Option<SecretString>, SecretError> {
        Ok(self.lock().get(name).cloned())
    }

    fn set(&self, name: &SecretRef, value: &SecretString) -> Result<(), SecretError> {
        self.lock().insert(name.clone(), value.clone());
        Ok(())
    }

    fn delete(&self, name: &SecretRef) -> Result<bool, SecretError> {
        Ok(self.lock().remove(name).is_some())
    }
}
