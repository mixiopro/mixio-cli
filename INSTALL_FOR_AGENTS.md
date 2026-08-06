# Install mixio-cli (for AI agents)

Paste everything between the rules into your AI coding agent — Claude Code, Codex, Gemini
CLI, Antigravity, or any agent with shell access, MCP client or not.

---

Set up the Mixio CLI for me.

**Step 1 — install the binary.** Detect my OS and run the matching command (prebuilt binary,
no Rust toolchain needed):

Linux / macOS:
```bash
curl -fsSL https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.sh | bash
```

Windows:
```powershell
powershell -c "irm https://raw.githubusercontent.com/mixiopro/mixio-cli/master/install.ps1 | iex"
```

Confirm it's on PATH with `mixio --help`. If that fails, the installer put it under
`~/.cargo/bin` — restart the shell or source the env file the installer printed.

**Step 2 — add a profile.** This needs an `sk-...` API key from Mixio Studio → Settings →
API Keys.

Do **not** run `mixio auth add <name> --key <value>` and do not ask me to paste the key into
this chat — a credential typed into an agent conversation is a credential that agent (and
this transcript) now holds. Instead, ask me to run this myself, in my own terminal, and wait
for me to confirm it's done before continuing:

```bash
mixio auth add myorg
```

It prompts for the key with hidden input — nothing about it reaches you.

**Step 3 — fetch the live tool schema and verify:**

```bash
mixio tools refresh
mixio call ping
mixio project list
```

Tell me what `mixio project list` returned so I know it's actually working, not just that the
commands exited zero.

---

## Using it as an agent

Every command is generated from the live MCP schema at runtime, so `mixio call --help` and
`mixio <noun> --help` are always accurate for whatever's deployed right now — check them
instead of guessing flags or arguments.

If you're following the [mixiopro/skills](https://github.com/mixiopro/skills) docs (which
name tools like `studio_list_projects`) but don't have an MCP client configured, translate
directly: strip the `studio_` prefix (that's a naming layer the local `@mixio-pro/mcp` proxy
adds, not part of the tool's real name) and it's a CLI command — `studio_list_projects` →
`mixio call list-projects`, or the shorter noun-verb form where one exists
(`mixio project list`). Run `mixio list-tools` for the full current set. The procedural
knowledge in those skills (what order to call things, what each field means, when to gate on
approval) still applies unchanged — only the transport differs.
