# Repository context

- **Link project scope:** `cli` — pass `--project cli` to Link commands to include company/global context and this repository's scoped memories.
- **Owning Plane project:** `MIXSTUDIO`
- **Repository:** `mixiopro/mixio-cli`
- **Purpose:** Mixio command-line client and packaging.
- **Entry points:**
  - [README.md](../README.md)
  - [Cargo.toml](../Cargo.toml)
  - [src/schema.rs](../src/schema.rs)
  - [src/groups.rs](../src/groups.rs)
  - [src/mcp.rs](../src/mcp.rs)
  - [install.sh](../install.sh)

> **Note:** Source entry points describe the maintained layout. If one is missing in an older or skeletal worktree, check that branch and the current source; do not invent files.

- **Boundaries:** This is a public client repository, not the Studio server. Verify exposed tool names and supported commands against the current client and server transport. Contributor tracking remains separate from end-user installation and usage; exclude credentials and private context from packages.

## Local links

- [Repository instructions](../AGENTS.md)
- [Maintainer tracking protocol](tracking-protocol.md)
- [Contributor operating skill](../.agents/skills/mixio-maintainer/SKILL.md)
