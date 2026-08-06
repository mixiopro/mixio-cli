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

**Setting this up via an AI coding agent instead?** Paste this into Claude Code, Codex,
Gemini CLI, Antigravity, or any agent with shell access — MCP client or not:

> Install the Mixio CLI for me. Detect my OS and run the matching command above (prebuilt
> binary, no Rust toolchain needed), then confirm with `mixio --help`.
>
> Then I need a profile, which needs an `sk-...` API key from Mixio Studio → Settings → API
> Keys. Do **not** run `mixio auth add <name> --key <value>` and do not ask me to paste the
> key into this chat — a credential typed into an agent conversation is a credential that
> agent now holds. Instead ask me to run `mixio auth add myorg` myself, in my own terminal —
> it prompts for the key with hidden input — and wait for me to confirm it's done.
>
> Once that's done, run `mixio tools refresh`, then `mixio call ping` and
> `mixio project list` to verify, and tell me what came back.

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

`mixio auth use <name>` switches the active profile; `mixio auth list` shows all of them.
Re-run `mixio tools refresh` after a Mixio deploy to pick up new/changed tools.

## Relationship to mixiopro/skills

[`mixiopro/skills`](https://github.com/mixiopro/skills) is the domain knowledge — what order
to call things in, what a field means, when to gate on approval. That's transport-agnostic;
it doesn't change whether the call happens over MCP or a shell command. This CLI is a second
front door onto the exact same backend, for consumers that don't have (or don't want) an MCP
client wired into their harness: humans at a terminal, CI/cron/scripts, or agents with only
shell access.

The one thing that *does* differ: the skills docs name tools like `studio_list_projects`,
because that prefix is added by the local `@mixio-pro/mcp` proxy when it forwards tools. The
raw hosted MCP endpoint this CLI talks to directly has no such prefix — strip it and you have
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

## Development

```bash
cargo test
cargo build
```

## License

Not yet decided — no `LICENSE` file in this repo yet. Do not treat the absence of a license as
permission to use, copy, or redistribute this code.
