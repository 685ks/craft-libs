use super::*;

fn name(text: &str) -> SecretRef {
    SecretRef::new(text).unwrap()
}

#[test]
fn memory_store_round_trip() {
    let store = MemoryStore::new();
    let key = name("openai.key");
    assert_eq!(store.get(&key).unwrap().map(|s| s.expose().to_owned()), None);
    store.set(&key, &SecretString::new("sk-one")).unwrap();
    store.set(&key, &SecretString::new("sk-two")).unwrap();
    assert_eq!(store.get(&key).unwrap().map(|s| s.expose().to_owned()).as_deref(), Some("sk-two"));
    assert_eq!(store.len(), 1);
    assert_eq!(store.delete(&key), Ok(true));
    assert_eq!(store.delete(&key), Ok(false));
    assert!(store.is_empty());
}

#[test]
fn memory_store_clones_share_and_debug_hides_values() {
    let store = MemoryStore::new();
    let other = store.clone();
    store.set(&name("a"), &SecretString::new("hunter2")).unwrap();
    assert!(other.get(&name("a")).unwrap().is_some());
    assert!(!format!("{store:?}").contains("hunter2"));
    other.clear();
    assert!(store.is_empty());
}

#[test]
fn apply_carries_out_key_changes() {
    let store = MemoryStore::new();
    let key = name("conn-1");
    store.apply(&key, &KeyChange::Set(SecretString::new("v1"))).unwrap();
    store.apply(&key, &KeyChange::Keep).unwrap();
    assert_eq!(store.get(&key).unwrap().map(|s| s.expose().to_owned()).as_deref(), Some("v1"));
    store.apply(&key, &KeyChange::Clear).unwrap();
    assert!(store.get(&key).unwrap().is_none());
    store.apply(&key, &KeyChange::Clear).unwrap();
}

#[test]
fn stores_work_through_arc_and_box_dyn() {
    let shared: Arc<dyn SecretStore> = Arc::new(MemoryStore::new());
    shared.set(&name("x"), &SecretString::new("1")).unwrap();
    let boxed: Box<dyn SecretStore> = Box::new(shared.clone());
    assert!(boxed.get(&name("x")).unwrap().is_some());
}

#[test]
fn memory_store_survives_a_poisoned_lock() {
    let store = MemoryStore::new();
    let poisoner = store.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poisoner.secrets.lock();
        panic!("poison the lock");
    })
    .join();
    store.set(&name("after"), &SecretString::new("ok")).unwrap();
    assert_eq!(store.len(), 1);
}

#[test]
fn errors_describe_without_secrets() {
    for e in [
        SecretError::Unavailable("no daemon".into()),
        SecretError::Denied("user said no".into()),
        SecretError::NotText,
        SecretError::TooLong("password is limited to 2560".into()),
        SecretError::Backend("oops".into()),
    ] {
        assert!(!e.to_string().is_empty());
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "windows", target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
#[test]
fn keyring_bad_encoding_never_carries_the_bytes() {
    let e = keychain::map_for_test(keyring::Error::BadEncoding(b"sk-secret-bytes".to_vec()));
    assert_eq!(e, SecretError::NotText);
    assert!(!format!("{e:?} {e}").contains("sk-secret"));
}

/// Talks to the real system keychain. Where none is reachable (a Linux CI runner without a
/// Secret Service, a locked keychain) the store must report an error, not panic or hang.
#[test]
fn keychain_round_trip_or_clean_error() {
    let store = KeychainStore::new("craft-secrets-test");
    assert_eq!(store.service(), "craft-secrets-test");
    let key = name(&format!("roundtrip-{}", std::process::id()));
    match store.set(&key, &SecretString::new("test-value-not-a-real-key")) {
        Ok(()) => {
            let back = store.get(&key).unwrap();
            assert_eq!(back.map(|s| s.expose().to_owned()).as_deref(), Some("test-value-not-a-real-key"));
            assert_eq!(store.delete(&key), Ok(true));
            assert_eq!(store.get(&key).map(|v| v.is_none()), Ok(true));
            assert_eq!(store.delete(&key), Ok(false));
        }
        Err(e) => {
            eprintln!("system keychain not usable here: {e}");
            assert!(!e.to_string().contains("test-value-not-a-real-key"));
            if !KeychainStore::is_supported() {
                assert!(matches!(e, SecretError::Unavailable(_)));
            }
        }
    }
}
