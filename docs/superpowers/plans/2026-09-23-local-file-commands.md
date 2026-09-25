# Local File Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `mixio file upload` and `mixio file url` as CLI-native parity for the stdio MCP's local-file tools.

**Architecture:** Add a deep `media` module with a small command-facing interface and the Inference Files/Studio upload implementation behind it. Keep the existing MCP client untouched; static commands use the active profile's API key directly because a remote MCP call cannot access a local path. Store a per-profile JSON cache keyed by canonical path and SHA-256.

**Tech Stack:** Rust 2021, clap, tokio, reqwest multipart/stream, serde/serde_json, sha2.

**Spec:** `docs/superpowers/specs/2026-09-23-file-commands.md`

## Global Constraints

- Follow the CLI's noun-first command convention: `mixio file upload` and `mixio file url`.
- Keep hosted MCP discovery/call behavior unchanged.
- Match the stdio bridge's supported media categories and upload flow.
- Never expose credentials in output or cache files.

### Task 1: Add media cache and upload client seams

**Files:**
- Create: `src/media.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces `MediaClient::upload(path, options)` and `MediaClient::public_url(path, options)` for the CLI handler.
- Produces JSON-serializable result types matching the stdio bridge's `ok`, `entry`, `public_url`, `source`, and cache-only fields.

- [x] Add failing unit tests for MCP URL-to-Studio-root conversion, supported categories, and cache scope matching.
- [x] Run `cargo test media` and confirm the new tests fail before implementation.
- [x] Add `sha2` and reqwest `multipart`/`stream` features.
- [x] Implement canonical-path validation, SHA-256 fingerprinting, per-profile JSON cache, MIME inference, cache lookup, and scope matching.
- [x] Implement small multipart upload, large-file upload reservation/presigned POST/confirmation, Studio media association, and CLI error propagation.
- [x] Implement `public_url` with cache-only mode and upload-on-miss mode.
- [x] Run `cargo test media` and confirm the focused tests pass.

### Task 2: Add CLI-native `file` commands

**Files:**
- Modify: `src/main.rs`
- Modify: `README.md`

**Interfaces:**
- Adds `mixio file upload <PATH>` with `--project-id`, `--organization-id`, `--alt`, `--category`, and `--force`.
- Adds `mixio file url <PATH>` with `--project-id`, `--organization-id`, `--alt`, `--category`, and `--no-upload`.

- [x] Add command-construction tests for positional paths, flags, category validation, and `--no-upload`.
- [x] Run `cargo test` and confirm the new command tests fail before wiring the commands.
- [x] Add a reserved static `file` noun so dynamic grouping cannot collide with it.
- [x] Wire the handlers through the active profile and `MediaClient`, printing pretty JSON.
- [x] Document examples and the hosted-URL versus local-file distinction.
- [x] Run targeted formatting, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build`.

Repository-wide `cargo fmt --check` was also run; it reports pre-existing formatting drift in untouched baseline files (including `src/cache.rs`, `src/groups.rs`, `src/profile.rs`, `src/schema.rs`, and `src/update_check.rs`).

### Task 3: Review and handoff

**Files:**
- Modify: none unless verification finds a defect.

- [x] Inspect the diff for credential/path leakage and accidental changes to dynamic MCP behavior.
- [x] Report implemented and verified status separately; do not claim merged or deployed.
