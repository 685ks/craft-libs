# craft-secrets

Secret storage behind `craft_ai_core::SecretRef`: the data model stores only the name of a
secret, and this crate keeps the value in the operating system's credential store.

- `SecretStore`: `get`, `set`, `delete`, and `apply` for a connection form's `KeyChange`.
  Values are `SecretString`, which cannot be printed or serialized by accident; no error ever
  carries a secret (the keyring crate's `BadEncoding` bytes are dropped).
- `KeychainStore::new(service)`: the `keyring` crate over the macOS Keychain, the Windows
  Credential Manager, or the Secret Service (GNOME Keyring, KWallet) via a pure-Rust D-Bus
  client on Linux and the BSDs, so no system library is needed to build. Each secret is the
  entry `(service, SecretRef)`; `craft` is the suite-wide service. Other targets (the web) get
  `SecretError::Unavailable` from every call.
- `MemoryStore`: in-memory secrets for tests and "don't remember this key" sessions.
