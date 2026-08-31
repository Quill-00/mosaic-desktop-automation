//! Credential vault. The cross-platform equivalent of Automata's DPAPI
//! `secrets.dat`: secrets (API keys, OAuth device-code tokens) live in the OS
//! keychain (Windows Credential Manager / macOS Keychain), never in db.json.

const SERVICE: &str = "mosaic";

pub fn set(key: &str, value: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, key)
        .and_then(|e| e.set_password(value))
        .map_err(|e| e.to_string())
}

pub fn get(key: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, key)
        .ok()
        .and_then(|e| e.get_password().ok())
}

pub fn delete(key: &str) -> Result<(), String> {
    match keyring::Entry::new(SERVICE, key) {
        Ok(e) => match e.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        },
        Err(e) => Err(e.to_string()),
    }
}

pub fn contains(key: &str) -> bool {
    get(key).is_some()
}

/// Vault key for a platform robot secret (QQ AppSecret in the current adapter).
pub fn bot_channel_key(id: &str) -> String {
    format!("bot-channel:{}", id)
}
