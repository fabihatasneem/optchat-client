//! Client tokens, one per connection (engine endpoint + workspace): in the shell's secure store
//! (optchat_platform::SecureStore: the Keychain on macOS and iOS), key `<endpoint>|<workspace>`. Never logged, never
//! in prefs. Tests set OPTCHAT_TOKEN_DIR: a JSON file there instead (0600 on unix; a rebuilt, unsigned binary would
//! make the Keychain ask for access, and tests must not answer system dialogs).
use std::collections::BTreeMap;

fn file() -> Option<String> {
    std::env::var("OPTCHAT_TOKEN_DIR").ok().map(|d| format!("{d}/tokens.json"))
}
fn read_file(p: &str) -> BTreeMap<String, String> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}
fn write_file(p: &str, m: &BTreeMap<String, String>) -> Result<(), String> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(p).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string(m).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

pub fn get(key: &str) -> Option<String> {
    if let Some(p) = file() {
        return read_file(&p).get(key).cloned();
    }
    optchat_platform::secure_store()?.get(key)
}
pub fn set(key: &str, token: &str) -> Result<(), String> {
    if let Some(p) = file() {
        let mut m = read_file(&p);
        m.insert(key.to_string(), token.to_string());
        return write_file(&p, &m);
    }
    optchat_platform::secure_store().ok_or_else(|| "no secure token store on this platform".to_string())?.set(key, token)
}
pub fn delete(key: &str) {
    if let Some(p) = file() {
        let mut m = read_file(&p);
        m.remove(key);
        let _ = write_file(&p, &m);
        return;
    }
    if let Some(s) = optchat_platform::secure_store() {
        s.delete(key);
    }
}

/// The local engine's client token (scope: every workspace), for an engine on this machine: chat.reframe.dev (the local
/// edge), localhost or 127.0.0.1. The file is the engine's (~/.reframe/data/workspaces/local-client-token, 0600); only
/// that one file is read, never logged or stored elsewhere. OPTCHAT_LOCAL_TOKEN_FILE points tests at their own file.
/// an engine on this machine: chat.reframe.dev (the local edge), localhost or 127.0.0.1
pub fn is_local(endpoint: &str) -> bool {
    let host = endpoint.split("://").nth(1).unwrap_or(endpoint).split(['/', ':']).next().unwrap_or("");
    matches!(host, "chat.reframe.dev" | "localhost" | "127.0.0.1")
}
pub fn local_token(endpoint: &str) -> Option<String> {
    if !is_local(endpoint) {
        return None;
    }
    let path = std::env::var("OPTCHAT_LOCAL_TOKEN_FILE").unwrap_or_else(|_| format!("{}/.reframe/data/workspaces/local-client-token", std::env::var("HOME").unwrap_or_default()));
    let t = std::fs::read_to_string(path).ok()?.trim().to_string();
    (!t.is_empty()).then_some(t)
}
