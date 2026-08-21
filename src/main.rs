mod cache;
mod groups;
mod mcp;
mod profile;
mod schema;
mod update_check;

use anyhow::{bail, Context, Result};
use clap::{Arg, Command};
use mcp::McpClient;
use serde_json::Value;

const RESERVED_NAMES: &[&str] = &["auth", "tools", "list-tools", "call", "help"];

#[tokio::main]
async fn main() -> Result<()> {
    // Runs concurrently with whatever command follows — free when cached
    // (no network call), bounded by a short timeout otherwise. Awaited at
    // the very end so it never adds latency to the actual command.
    let update_check = update_check::spawn();

    let profile_name = profile::active().ok().map(|p| p.name);
    let cached = profile_name.as_deref().and_then(cache::load);
    let groups = cached.as_ref().map(|c| groups::derive(&c.tools)).unwrap_or_default();

    let mut call_cmd = Command::new("call").about(match &cached {
        Some(c) => format!("Call an MCP tool by name ({} cached)", c.tools.len()),
        None => "No tools cached yet — run `mixio auth add <name>` then `mixio tools refresh`".into(),
    });
    if let Some(c) = &cached {
        for tool in &c.tools {
            call_cmd = call_cmd.subcommand(schema::build_tool_command(tool).name(schema::to_kebab(&tool.name)));
        }
    }

    let mut cli = Command::new("mixio")
        .about("Mixio Studio CLI — profiles, and a dynamic client for the hosted MCP tool surface")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(auth_command())
        .subcommand(
            Command::new("tools")
                .about("Manage the cached MCP tool schema")
                .subcommand_required(true)
                .subcommand(Command::new("refresh").about("Re-fetch tools/list from the active profile's MCP endpoint")),
        )
        .subcommand(Command::new("list-tools").about("List cached MCP tools"))
        .subcommand(call_cmd);

    // Derived `mixio <noun> <verb>` shortcuts — see groups.rs for why a noun
    // only appears here once it clears the collision + min-group-size bars.
    for (noun, verbs) in &groups {
        if RESERVED_NAMES.contains(&noun.as_str()) {
            continue; // defensive; no observed collision today
        }
        let mut noun_cmd = Command::new(noun.clone()).about(format!("{noun} operations ({} available)", verbs.len()));
        for (verb, tool_name) in verbs {
            if let Some(tool) = cached.as_ref().and_then(|c| c.tools.iter().find(|t| &t.name == tool_name)) {
                noun_cmd = noun_cmd.subcommand(schema::build_tool_command(tool).name(verb.clone()));
            }
        }
        cli = cli.subcommand(noun_cmd);
    }

    let matches = cli.get_matches();
    match matches.subcommand() {
        Some(("auth", sub)) => handle_auth(sub)?,
        Some(("tools", sub)) => handle_tools(sub).await?,
        Some(("list-tools", _)) => handle_list_tools(&cached, &groups)?,
        Some(("call", sub)) => {
            let Some((display_name, tool_matches)) = sub.subcommand() else {
                bail!("specify a tool — see `mixio call --help`");
            };
            // Subcommands are named kebab-case for display; resolve back to
            // the server's real tool name via the same cache the command
            // tree was built from — no assumption that the transform is
            // cleanly reversible (a tool name could someday already contain
            // a hyphen).
            let tool_name = cached
                .as_ref()
                .and_then(|c| c.tools.iter().find(|t| schema::to_kebab(&t.name) == display_name))
                .with_context(|| format!("unknown tool `{display_name}` — run `mixio tools refresh`"))?
                .name
                .clone();
            invoke(&cached, &tool_name, tool_matches).await?;
        }
        Some((noun, sub)) => {
            let Some((verb, verb_matches)) = sub.subcommand() else {
                bail!("specify a verb — see `mixio {noun} --help`");
            };
            let tool_name = groups
                .get(noun)
                .and_then(|verbs| verbs.get(verb))
                .with_context(|| format!("`mixio {noun} {verb}` isn't wired to a tool — this is a bug"))?;
            invoke(&cached, tool_name, verb_matches).await?;
        }
        None => unreachable!("subcommand_required"),
    }

    if let Ok(Some(notice)) = update_check.await {
        eprintln!("{notice}");
    }
    Ok(())
}

fn auth_command() -> Command {
    Command::new("auth")
        .about("Manage profiles (one per org/account)")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .about("Add a profile, storing its API key")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("key").long("key").help("sk-... API key; prompted if omitted"))
                .arg(
                    Arg::new("base-url")
                        .long("base-url")
                        .default_value(profile::DEFAULT_BASE_URL),
                ),
        )
        .subcommand(Command::new("use").about("Switch the active profile").arg(Arg::new("name").required(true)))
        .subcommand(Command::new("list").about("List profiles"))
        .subcommand(Command::new("whoami").about("Show the active profile"))
        .subcommand(Command::new("remove").about("Remove a profile").arg(Arg::new("name").required(true)))
}

fn handle_auth(sub: &clap::ArgMatches) -> Result<()> {
    match sub.subcommand() {
        Some(("add", m)) => {
            let name = m.get_one::<String>("name").unwrap();
            let key = match m.get_one::<String>("key") {
                Some(k) => k.clone(),
                None => rpassword::prompt_password("Mixio API key (sk-...): ")?,
            };
            let base_url = m.get_one::<String>("base-url").cloned();
            profile::add(name, &key, base_url)?;
            println!("added profile `{name}`");
        }
        Some(("use", m)) => {
            let name = m.get_one::<String>("name").unwrap();
            profile::use_profile(name)?;
            println!("switched to `{name}`");
        }
        Some(("list", _)) => {
            let profiles = profile::list()?;
            if profiles.is_empty() {
                println!("no profiles — run `mixio auth add <name>`");
            }
            for (name, meta, is_active) in profiles {
                println!("{} {name}\t{}", if is_active { "*" } else { " " }, meta.base_url);
            }
        }
        Some(("whoami", _)) => {
            println!("{}", profile::active()?.name);
        }
        Some(("remove", m)) => {
            let name = m.get_one::<String>("name").unwrap();
            profile::remove(name)?;
            println!("removed `{name}`");
        }
        _ => unreachable!("subcommand_required"),
    }
    Ok(())
}

async fn handle_tools(sub: &clap::ArgMatches) -> Result<()> {
    match sub.subcommand() {
        Some(("refresh", _)) => {
            let profile = profile::active()?;
            let mut client = McpClient::new(&profile.base_url, &profile.api_key);
            client.initialize().await?;
            let tools = client.list_tools().await?;
            cache::save(&profile.name, &tools)?;
            println!("cached {} tools for profile `{}`", tools.len(), profile.name);
        }
        _ => unreachable!("subcommand_required"),
    }
    Ok(())
}

fn handle_list_tools(cached: &Option<cache::CachedTools>, groups: &groups::Groups) -> Result<()> {
    let cached = cached.as_ref().context("no cached tools — run `mixio tools refresh`")?;
    warn_if_stale(cached);
    for tool in &cached.tools {
        let display = schema::to_kebab(&tool.name);
        match alias_for(groups, &tool.name) {
            Some(alias) => println!("{display} (mixio {alias})\t{}", tool.description),
            None => println!("{display}\t{}", tool.description),
        }
    }
    Ok(())
}

/// The `mixio <noun> <verb>` shortcut for a raw tool name, if `groups.rs`
/// derived one — same relationship `call` and the grouped commands already
/// share, just surfaced where a reader would actually be confused: skills
/// docs name tools verb-first (`get_project`, matching the MCP convention),
/// this CLI's grouped shortcuts are noun-first (`mixio project get`,
/// matching CLI convention). Same tool, two calling conventions for two
/// different audiences — this makes that undeniable instead of surprising.
fn alias_for(groups: &groups::Groups, tool_name: &str) -> Option<String> {
    groups.iter().find_map(|(noun, verbs)| {
        verbs.iter().find_map(|(verb, raw)| (raw == tool_name).then(|| format!("{noun} {verb}")))
    })
}

/// Shared by `mixio call <tool>` and every derived `mixio <noun> <verb>` —
/// both just need to resolve a raw tool name and a matched arg set.
async fn invoke(cached: &Option<cache::CachedTools>, tool_name: &str, tool_matches: &clap::ArgMatches) -> Result<()> {
    let cached = cached.as_ref().context("no cached tools — run `mixio tools refresh`")?;
    warn_if_stale(cached);
    let tool = cached
        .tools
        .iter()
        .find(|t| t.name == tool_name)
        .with_context(|| format!("unknown tool `{tool_name}` — run `mixio tools refresh`"))?;
    let args = schema::collect_arguments(tool, tool_matches)?;

    let profile = profile::active()?;
    let mut client = McpClient::new(&profile.base_url, &profile.api_key);
    client.initialize().await?;
    let result = client.call_tool(tool_name, args).await?;

    let rendered = render_result(&result)?;
    if result.get("isError").and_then(serde_json::Value::as_bool) == Some(true) {
        bail!("tool reported an error:\n{rendered}");
    }
    println!("{rendered}");
    Ok(())
}

/// MCP wraps tool output as `content: [{ type, text }, ...]`; several Mixio
/// tools put a JSON string inside `text`, so the naive pretty-print showed
/// JSON escaped a second time. Unwrap and re-pretty-print those blocks;
/// anything that doesn't match this exact shape (mixed content, images, a
/// plain non-JSON message) falls back to the raw envelope untouched.
fn render_result(result: &Value) -> Result<String> {
    if let Some(text) = unwrap_text_content(result) {
        return Ok(text);
    }
    Ok(serde_json::to_string_pretty(result)?)
}

fn unwrap_text_content(result: &Value) -> Option<String> {
    let blocks = result.get("content")?.as_array()?;
    if blocks.is_empty() {
        return None;
    }
    let mut parts = Vec::with_capacity(blocks.len());
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("text") {
            return None; // non-text content block — show the raw envelope instead
        }
        let text = block.get("text")?.as_str()?;
        let rendered = serde_json::from_str::<Value>(text)
            .and_then(|v| serde_json::to_string_pretty(&v))
            .unwrap_or_else(|_| text.to_string());
        parts.push(rendered);
    }
    Some(parts.join("\n"))
}

fn warn_if_stale(cached: &cache::CachedTools) {
    if cached.is_stale(cache::DEFAULT_TTL_SECS) {
        eprintln!("warning: tool cache is stale — run `mixio tools refresh`");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_whoami_is_exposed() {
        let matches = auth_command().try_get_matches_from(["auth", "whoami"]).unwrap();
        assert_eq!(matches.subcommand_name(), Some("whoami"));
    }
}
