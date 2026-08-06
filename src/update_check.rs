//! Lightweight, rate-limited "a newer version exists" notice — this never
//! modifies anything itself. The actual update mechanism is the
//! `mixio-update` companion binary cargo-dist installs alongside `mixio`
//! (`install-updater` in dist-workspace.toml); this just tells you when to
//! run it.
//!
//! Never blocks and never prompts: `mixio` has to work unattended in CI and
//! for agents, so a version check that waits on stdin the first time it
//! fires would hang a script. The one place this differs by severity is the
//! message text, not control flow.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CHECK_INTERVAL_SECS: u64 = 86_400; // once a day
const REQUEST_TIMEOUT: Duration = Duration::from_millis(800);
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/mixiopro/mixio-cli/releases/latest";

#[derive(Serialize, Deserialize)]
struct Cached {
    checked_at: u64,
    latest: String,
}

/// Spawns the check concurrently with whatever command is actually running.
/// Await the result at the very end of `main` — free when nothing needs
/// checking (cached, no network call), bounded by `REQUEST_TIMEOUT`
/// otherwise. Never errors: a broken update check must never break the CLI.
pub fn spawn() -> tokio::task::JoinHandle<Option<String>> {
    tokio::spawn(async { check().await.unwrap_or(None) })
}

async fn check() -> anyhow::Result<Option<String>> {
    let path = cache_path()?;
    let latest = match read_cache(&path).filter(|c| !is_stale(c.checked_at)) {
        Some(cached) => cached.latest,
        None => {
            let latest = fetch_latest().await?;
            let _ = write_cache(&path, &Cached { checked_at: now_secs(), latest: latest.clone() });
            latest
        }
    };
    Ok(notice(env!("CARGO_PKG_VERSION"), &latest))
}

async fn fetch_latest() -> anyhow::Result<String> {
    let client = reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build()?;
    let body: serde_json::Value = client
        .get(LATEST_RELEASE_API)
        .header("User-Agent", "mixio-cli")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    body.get("tag_name")
        .and_then(serde_json::Value::as_str)
        .map(|s| s.trim_start_matches('v').to_string())
        .context("release response had no tag_name")
}

fn notice(current: &str, latest: &str) -> Option<String> {
    if current == latest {
        return None;
    }
    let url = format!("https://github.com/mixiopro/mixio-cli/releases/tag/v{latest}");
    if is_breaking(current, latest) {
        Some(format!(
            "mixio {latest} is available and may change CLI behavior ({current} -> {latest}) — \
             check {url} before running `mixio-update`"
        ))
    } else {
        Some(format!("a newer mixio is available: {current} -> {latest} — run `mixio-update`"))
    }
}

/// While major is 0, a minor bump is the one semver allows to break things
/// (`0.y.z`); once major reaches 1+, a major bump is the signal instead.
/// Unparseable versions are treated as potentially breaking — silently
/// downgrading a warning because two version strings didn't parse would
/// defeat the point of having one.
fn is_breaking(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some((0, cur_minor, _)), Some((0, lat_minor, _))) => cur_minor != lat_minor,
        (Some((cur_major, _, _)), Some((lat_major, _, _))) => cur_major != lat_major,
        _ => true,
    }
}

fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn is_stale(checked_at: u64) -> bool {
    now_secs().saturating_sub(checked_at) > CHECK_INTERVAL_SECS
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn cache_path() -> anyhow::Result<std::path::PathBuf> {
    let base = dirs::cache_dir().context("could not determine cache directory")?;
    Ok(base.join("mixio").join("update-check.json"))
}

fn read_cache(path: &std::path::Path) -> Option<Cached> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write_cache(path: &std::path::Path, cached: &Cached) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(cached)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_1_0_treats_minor_bump_as_breaking() {
        assert!(is_breaking("0.0.3", "0.1.0"));
        assert!(!is_breaking("0.0.3", "0.0.4"));
    }

    #[test]
    fn post_1_0_treats_major_bump_as_breaking() {
        assert!(is_breaking("1.2.3", "2.0.0"));
        assert!(!is_breaking("1.2.3", "1.3.0"));
        assert!(!is_breaking("1.2.3", "1.2.4"));
    }

    #[test]
    fn same_version_has_no_notice() {
        assert_eq!(notice("0.0.3", "0.0.3"), None);
    }

    #[test]
    fn unparseable_version_defaults_to_breaking() {
        assert!(is_breaking("not-a-version", "0.0.4"));
    }
}
