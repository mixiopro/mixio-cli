# mixio-cli

[![Release](https://github.com/mixiopro/mixio-cli/actions/workflows/release.yml/badge.svg)](https://github.com/mixiopro/mixio-cli/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/mixiopro/mixio-cli)](https://github.com/mixiopro/mixio-cli/releases/latest)

A CLI for [Mixio Studio](https://mixio.studio) — profiles for multiple orgs/accounts, and a
dynamic client for the hosted MCP tool surface at `studio.mixio.pro/api/mcp`.

Every command is derived from the MCP server's live `tools/list` schema at runtime — no
hand-written subcommand per tool, so it stays in sync automatically as the server's tool
surface changes across deploys. See [How it works](#how-it-works).

## Install

**Linux & macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.sh | bash
```

**Windows:**
```powershell
powershell -c "irm https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.ps1 | iex"
```

Prebuilt binary for Linux (x86_64/aarch64), macOS (Intel/Apple Silicon), and Windows
(x86_64) — no Rust toolchain required. Building from source instead:
```bash
cargo install --path .
```

## Quick start

1. **Get an API key** — Mixio Studio → Settings → API Keys → Create Key (`sk-...`).
2. **Add a profile** (one per org/account — the key is already org-scoped server-side, so
   switching profiles is how you switch orgs):
   ```bash
   mixio auth add myorg     # prompts for the key with hidden input
   ```
3. **Fetch the live tool schema:**
   ```bash
   mixio tools refresh
   ```
4. **See what's available, then call something:**
   ```bash
   mixio list-tools
   mixio call --help

   # every tool by its raw name
   mixio call list-projects --limit 10

   # or through a friendly noun/verb group, where one was cleanly derivable
   mixio project list
   mixio project create --title "My Project"
   ```

`mixio auth use <name>` switches the active profile; `mixio auth whoami` shows the active profile name; `mixio auth list` shows all of them. `mixio upgrade` installs the latest CLI release.
Re-run `mixio tools refresh` after a Mixio deploy to pick up new/changed tools.

## For AI agents

### Just ask the agent

Every command is self-documenting straight from the live schema, so an agent doesn't need to
have read this README — it discovers everything by running the CLI itself, via `--help`. One
paste installs it and sets up persistent instructions in your agent's memory file:

> Install mixio-cli: detect my OS and run the matching one-liner from the Install section of
> https://github.com/mixiopro/mixio-cli, then confirm with `mixio --help`. Add a "## Mixio"
> section to my AGENTS.md or CLAUDE.md: use `mixio` for Mixio Studio (projects, episodes,
> generation jobs) — `mixio --help` for commands, `mixio <noun> --help` for a resource's
> operations, `mixio call --help` for the full tool set by raw name. Then check whether a
> profile exists (`mixio auth list`); if not, ask me to run `mixio auth add <name>` myself, in
> my own terminal, and wait for me to confirm — never run that with `--key`, and never ask me
> to paste an API key into this chat.

The credential rule is the one thing worth stating explicitly every time: an agent has no way
to infer on its own that it shouldn't handle the key, since nothing about `mixio auth add`
looks different from any other setup command until it prompts for one.

### Coming from mixiopro/skills

[`mixiopro/skills`](https://github.com/mixiopro/skills) is the domain knowledge for
MCP-native agents (Claude Code, Cursor, etc. with an MCP client configured) — what order to
call things in, what a field means, when to gate on approval. That knowledge is
transport-agnostic; only the tool names differ. The skills docs say `studio_list_projects`
because that prefix is added by the local `@mixio-pro/mcp` proxy when it forwards tools — the
raw hosted MCP endpoint this CLI talks to directly has no such prefix. Strip it and you have
the CLI command: `studio_list_projects` → `mixio call list-projects`, or the shorter
noun-verb form where one was cleanly derivable (`mixio project list`; see
[How it works](#how-it-works) for what "cleanly derivable" means). `mixio list-tools` shows
the full current set.

## How it works

MCP's `tools/list` gives every tool's name, description, and JSON Schema — enough to build a
real CLI at runtime with no codegen step:

- **`src/schema.rs`** maps each tool's JSON Schema straight to a `clap::Command`/`Arg` tree —
  types, enums, required flags, and `default`s become typed flags with real `--help` text.
- **`src/groups.rs`** derives `mixio <noun> <verb>` shortcuts (e.g. `project list` for
  `list_projects`) from a small, stable verb vocabulary — not from hardcoded tool names, so a
  server-side rename can't leave a shortcut silently pointing at nothing. Two guardrails keep
  the guess honest: a (noun, verb) pair claimed by more than one tool is aliased to neither
  (two tools can share a name shape and still be genuinely different), and a noun with only
  one verb doesn't form a group (not worth inventing structure for).
- `mixio call <raw-tool-name>` is always available as the ground-truth escape hatch, including
  for anything the grouping heuristic declined to alias.
- The raw tool name and the grouped alias use different word orders on purpose — MCP tools
  read verb-first (`get_project`, matching how the server and `mixiopro/skills` name them),
  the CLI shortcut reads noun-first (`mixio project get`, matching CLI convention, same as
  `gh`/`docker`). `mixio list-tools` shows both side by side for exactly this reason.

## Development

```bash
cargo test
cargo build
```

## License

Not yet decided — no `LICENSE` file in this repo yet. Do not treat the absence of a license as
permission to use, copy, or redistribute this code.
