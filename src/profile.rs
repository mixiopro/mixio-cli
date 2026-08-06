//! Named profiles, gcloud-style: each holds one Mixio API key (already
//! org-scoped server-side) and the MCP endpoint it talks to. "Switching
//! orgs" is switching the active profile — no org API needed client-side.
//!
//! Two files, deliberately separate: `config.json` (names, endpoints, which
//! one is active) is fine to read/sync/back up; `credentials.json` holds the
//! actual `sk-...` keys and is written 0600.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const DEFAULT_BASE_URL: &str = "https://studio.mixio.pro/api/mcp";

#[derive(Serialize, Deserialize, Default)]
struct Config {
    active: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileMeta>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProfileMeta {
    pub base_url: String,
}

pub struct Profile {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

pub fn add(name: &str, api_key: &str, base_url: Option<String>) -> Result<()> {
    let mut config = load_config()?;
    let make_active = config.active.is_none();
    config.profiles.insert(
        name.to_string(),
        ProfileMeta { base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()) },
    );
    if make_active {
        config.active = Some(name.to_string());
    }
    save_config(&config)?;

    let mut creds = load_credentials()?;
    creds.insert(name.to_string(), api_key.to_string());
    save_credentials(&creds)
}

pub fn use_profile(name: &str) -> Result<()> {
    let mut config = load_config()?;
    if !config.profiles.contains_key(name) {
        bail!("no such profile `{name}` — run `mixio auth list`");
    }
    config.active = Some(name.to_string());
    save_config(&config)
}

pub fn remove(name: &str) -> Result<()> {
    let mut config = load_config()?;
    if config.profiles.remove(name).is_none() {
        bail!("no such profile `{name}`");
    }
    if config.active.as_deref() == Some(name) {
        config.active = config.profiles.keys().next().cloned();
    }
    save_config(&config)?;

    let mut creds = load_credentials()?;
    creds.remove(name);
    save_credentials(&creds)
}

pub fn list() -> Result<Vec<(String, ProfileMeta, bool)>> {
    let config = load_config()?;
    Ok(config
        .profiles
        .into_iter()
        .map(|(name, meta)| {
            let is_active = config.active.as_deref() == Some(name.as_str());
            (name, meta, is_active)
        })
        .collect())
}

/// The active profile, with its key loaded — what every MCP call needs.
pub fn active() -> Result<Profile> {
    let config = load_config()?;
    let name = config
        .active
        .context("no active profile — run `mixio auth add <name>` first")?;
    let meta = config
        .profiles
        .get(&name)
        .with_context(|| format!("active profile `{name}` has no stored config"))?
        .clone();
    let creds = load_credentials()?;
    let api_key = creds
        .get(&name)
        .with_context(|| format!("active profile `{name}` has no stored API key"))?
        .clone();
    Ok(Profile { name, base_url: meta.base_url, api_key })
}

fn config_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir().context("could not determine config directory")?.join("mixio"))
}

fn load_config() -> Result<Config> {
    let path = config_dir()?.join("config.json");
    match std::fs::read(&path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).context("reading config.json"),
    }
}

fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("config.json"), serde_json::to_vec_pretty(config)?)?;
    Ok(())
}

fn load_credentials() -> Result<BTreeMap<String, String>> {
    let path = config_dir()?.join("credentials.json");
    match std::fs::read(&path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e).context("reading credentials.json"),
    }
}

// ponytail: plain 0600 file, matches `gh`'s default. Upgrade to an OS
// keychain (`keyring` crate) if that ever needs to be stronger than "not
// world-readable on this machine".
fn save_credentials(creds: &BTreeMap<String, String>) -> Result<()> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("credentials.json");
    std::fs::write(&path, serde_json::to_vec_pretty(creds)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
