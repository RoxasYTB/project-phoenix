use std::sync::Mutex;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

pub trait TokenStore: Send + Sync + 'static {
    fn load(&self) -> Result<Option<Secret>, String>;
    fn save(&self, token: &Secret) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
    fn load_account(&self, _id: u64) -> Result<Option<Secret>, String> {
        Ok(None)
    }
    fn save_account(&self, _id: u64, _token: &Secret) -> Result<(), String> {
        Ok(())
    }
    fn clear_account(&self, _id: u64) -> Result<(), String> {
        Ok(())
    }
}

pub fn account_key(id: u64) -> String {
    format!("account:{id}")
}

pub const SERVICE: &str = "discord-phoenix";
pub const ACCOUNT: &str = "token";

#[derive(Debug, Default)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
    fn account_entry(id: u64) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, &account_key(id)).map_err(|e| e.to_string())
    }
}

impl TokenStore for KeyringStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        if crate::paths::is_sandboxed() {
            return Ok(None);
        }
        match Self::entry()?.get_password() {
            Ok(t) if !t.is_empty() => Ok(Some(Secret(t))),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, token: &Secret) -> Result<(), String> {
        if crate::paths::is_sandboxed() {
            return Err("keyring disabled in sandbox".into());
        }
        Self::entry()?.set_password(token.expose()).map_err(|e| e.to_string())
    }
    fn clear(&self) -> Result<(), String> {
        if crate::paths::is_sandboxed() {
            return Ok(());
        }
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn load_account(&self, id: u64) -> Result<Option<Secret>, String> {
        if crate::paths::is_sandboxed() {
            return Ok(None);
        }
        match Self::account_entry(id)?.get_password() {
            Ok(t) if !t.is_empty() => Ok(Some(Secret(t))),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save_account(&self, id: u64, token: &Secret) -> Result<(), String> {
        if crate::paths::is_sandboxed() {
            return Err("keyring disabled in sandbox".into());
        }
        let e = Self::account_entry(id)?;
        if matches!(e.get_password(), Ok(t) if t == token.expose()) {
            return Ok(());
        }
        e.set_password(token.expose()).map_err(|e| e.to_string())
    }
    fn clear_account(&self, id: u64) -> Result<(), String> {
        if crate::paths::is_sandboxed() {
            return Ok(());
        }
        match Self::account_entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryStore(pub Mutex<Option<Secret>>);

impl TokenStore for MemoryStore {
    fn load(&self) -> Result<Option<Secret>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, token: &Secret) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(token.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_is_never_touched_in_tests() {
        let k = KeyringStore;
        assert_eq!(k.load(), Ok(None));
        assert!(k.save(&Secret::new("x")).is_err());
        assert_eq!(k.clear(), Ok(()));
    }

    #[test]
    fn secret_debug_is_redacted() {
        let s = Secret::new("mfa.very-secret-token");
        assert!(!format!("{s:?}").contains("very-secret"));
    }

    #[test]
    fn memory_store_roundtrip() {
        let st = MemoryStore::default();
        assert!(st.load().unwrap().is_none());
        st.save(&Secret::new("t")).unwrap();
        assert_eq!(st.load().unwrap().unwrap().expose(), "t");
        st.clear().unwrap();
        assert!(st.load().unwrap().is_none());
    }
}
