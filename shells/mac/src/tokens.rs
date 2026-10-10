//! Client tokens, one per connection (engine endpoint + workspace): in the macOS Keychain (generic passwords, service
//! `com.reframe.optchat.gpui`, account `<endpoint>|<workspace>`). Never logged, never in prefs.
//! Tests set OPTCHAT_TOKEN_DIR: a 0600 JSON file there instead (a rebuilt, unsigned binary would make the Keychain ask
//! for access, and tests must not answer system dialogs).
use std::collections::BTreeMap;

const SERVICE: &str = "com.reframe.optchat.gpui";

fn file() -> Option<String> {
    std::env::var("OPTCHAT_TOKEN_DIR").ok().map(|d| format!("{d}/tokens.json"))
}
fn read_file(p: &str) -> BTreeMap<String, String> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_file(p: &str, m: &BTreeMap<String, String>) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string(m).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

pub fn get(key: &str) -> Option<String> {
    if let Some(p) = file() {
        return read_file(&p).get(key).cloned();
    }
    security_framework::passwords::get_generic_password(SERVICE, key).ok().and_then(|b| String::from_utf8(b).ok())
}
pub fn set(key: &str, token: &str) -> Result<(), String> {
    if let Some(p) = file() {
        let mut m = read_file(&p);
        m.insert(key.to_string(), token.to_string());
        return write_file(&p, &m);
    }
    security_framework::passwords::set_generic_password(SERVICE, key, token.as_bytes()).map_err(|e| format!("Keychain: {e}"))
}
pub fn delete(key: &str) {
    if let Some(p) = file() {
        let mut m = read_file(&p);
        m.remove(key);
        let _ = write_file(&p, &m);
        return;
    }
    let _ = security_framework::passwords::delete_generic_password(SERVICE, key);
}

#[cfg(test)]
mod tests {
    /// the Keychain round trip (an item this test binary creates is readable by it without a prompt)
    #[::core::prelude::v1::test]
    fn keychain_round_trip() {
        if std::env::var("OPTCHAT_TOKEN_DIR").is_ok() {
            return;
        }
        let k = format!("test|roundtrip-{}", std::process::id());
        super::set(&k, "not-a-real-token").unwrap();
        assert_eq!(super::get(&k).as_deref(), Some("not-a-real-token"));
        super::delete(&k);
        assert_eq!(super::get(&k), None);
    }
}
