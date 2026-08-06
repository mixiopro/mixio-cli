# mixio-cli

A CLI for [Mixio Studio](https://mixio.studio) — profiles for multiple orgs/accounts, and a
dynamic client for the hosted MCP tool surface at `studio.mixio.pro/api/mcp`.

Every command is derived from the MCP server's live `tools/list` schema at runtime — no
hand-written subcommand per tool, so it stays in sync automatically as the server's tool
surface changes across deploys. See [`src/schema.rs`](src/schema.rs) and
[`src/groups.rs`](src/groups.rs) for how.

## Install

**Linux & macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.sh | bash
```

**Windows:**
```powershell
powershell -c "irm https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.ps1 | iex"
```

Prebuilt binary, no Rust toolchain required. Building from source instead:
```bash
cargo install --path .
```

## Usage

```bash
# add a profile (one per org/account), prompts for the sk-... API key
mixio auth add myorg

# fetch the live tool schema and cache it
mixio tools refresh

# see what's available
mixio list-tools
mixio call --help

# call any tool directly by name
mixio call list-projects --limit 10

# or through a friendly noun/verb group, where one was cleanly derivable
# (see groups.rs for the two guardrails that decide "cleanly")
mixio project list
mixio project create --title "My Project"
```

Switch profiles with `mixio auth use <name>`; `mixio auth list` shows all of them.

## Development

```bash
cargo test
cargo build
```
