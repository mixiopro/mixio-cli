//! Local cache of `tools/list`, one file per profile — different profiles
//! (orgs) can see different tool sets, and it lets `mixio call --help` work
//! without a network round-trip every time. `mixio tools refresh` forces a
//! re-fetch; ttl-based staleness triggers an automatic one otherwise.

use crate::mcp::McpTool;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_TTL_SECS: u64 = 3600;

#[derive(Serialize, Deserialize)]
pub struct CachedTools {
    pub fetched_at: u64,
    pub tools: Vec<McpTool>,
}

impl CachedTools {
    pub fn is_stale(&self, ttl_secs: u64) -> bool {
        now_secs().saturating_sub(self.fetched_at) > ttl_secs
    }
}

pub fn load(profile: &str) -> Option<CachedTools> {
    let bytes = std::fs::read(cache_path(profile).ok()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(profile: &str, tools: &[McpTool]) -> Result<()> {
    let path = cache_path(profile)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    let cached = CachedTools { fetched_at: now_secs(), tools: tools.to_vec() };
    std::fs::write(path, serde_json::to_vec_pretty(&cached)?)?;
    Ok(())
}

fn cache_path(profile: &str) -> Result<PathBuf> {
    let base = dirs::cache_dir().context("could not determine cache directory")?;
    Ok(base.join("mixio").join(format!("tools-{profile}.json")))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
