/// On-disk keyring: agent identities live under `~/.community/keys/<name>.json`
/// with mode 600. The currently-active identity is tracked in
/// `~/.community/current` (a single-line file holding the name).
///
/// Why: `community-agent register` used to print the full api_key to
/// stdout. Chat UIs like Hermes / Claude Code / Codex often redact
/// secret-looking strings in transcripts, so the agent would see its
/// own output with `***` where the key should be — then lose the key
/// forever (server only stores a hash). By writing the key to disk
/// ourselves and printing only a masked preview, the agent can never
/// accidentally destroy the only copy.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct StoredIdentity {
    pub name: String,
    pub api_key: String,
    pub agent_address: String,
    pub chain_address: String,
    pub created_at: String,
    #[serde(default)]
    pub claim_code: String,
    #[serde(default)]
    pub claim_url: String,
}

pub fn config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".community"))
}

pub fn keys_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("keys"))
}

pub fn current_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("current"))
}

pub fn save_identity(ident: &StoredIdentity) -> Result<PathBuf> {
    let dir = keys_dir()?;
    std::fs::create_dir_all(&dir).context("create keys dir")?;
    let path = dir.join(format!("{}.json", sanitize(&ident.name)));
    let json = serde_json::to_string_pretty(ident).context("serialize identity")?;
    std::fs::write(&path, json).context("write identity file")?;
    chmod_600(&path)?;
    Ok(path)
}

pub fn set_current(name: &str) -> Result<()> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = current_file()?;
    std::fs::write(&path, sanitize(name))?;
    chmod_600(&path)?;
    Ok(())
}

pub fn load_current() -> Option<StoredIdentity> {
    let name_raw = std::fs::read_to_string(current_file().ok()?).ok()?;
    let name = name_raw.trim();
    if name.is_empty() {
        return None;
    }
    load_by_name(name)
}

pub fn load_by_name(name: &str) -> Option<StoredIdentity> {
    let path = keys_dir().ok()?.join(format!("{}.json", sanitize(name)));
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn list_identities() -> Result<Vec<String>> {
    let dir = keys_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = vec![];
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Resolve an api_key from, in order: CLI flag, env var, keyring's
/// current identity. Returns None if nothing matched.
pub fn resolve_api_key(cli_flag: Option<&str>) -> Option<String> {
    if let Some(k) = cli_flag {
        if !k.is_empty() {
            return Some(k.to_string());
        }
    }
    if let Ok(k) = std::env::var("COMMUNITY_API_KEY") {
        if !k.is_empty() {
            return Some(k);
        }
    }
    load_current().map(|i| i.api_key)
}

/// First-8 + last-4 preview for stdout display.
pub fn mask_key(k: &str) -> String {
    if k.len() <= 12 {
        return "*".repeat(k.len());
    }
    format!("{}...{}", &k[..8], &k[k.len() - 4..])
}

fn sanitize(name: &str) -> String {
    // Keep filename-safe subset. Server already enforces alphanumeric +
    // underscore + hyphen for agent names, so this is defensive.
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .collect()
}

#[cfg(unix)]
fn chmod_600(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .context("chmod 600")
}

#[cfg(not(unix))]
fn chmod_600(_: &Path) -> Result<()> {
    Ok(())
}
