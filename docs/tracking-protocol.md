# Maintainer tracking protocol

Protocol version: `2026-09-11.1` · Governance ticket: `MIXSTUDIO-501`.

This protocol applies to Mixio maintainer engineering and governance work. Public skill users and normal creative production do not need private Plane access. In production/sample workspaces (`avgc-mcp-setup`, `mixio-agent-setup`), contributor changes to skills, setup, or tooling belong to MIXSKILLS; making a film or using installed skills follows the production workflow. In `founder-office`, this protocol covers product engineering only; existing local reference context and Outline financial, commercial, and operating records retain their ownership.

## 1. Start: read the live scope

Before implementation, read the live Plane ticket, its state, acceptance criteria, dependencies, and recent updates. Resolve its project against `config/tracking-repositories.json` in the configured shared tracking checkout. Record the ticket identifier, repository, branch/worktree, and acceptance criteria in the working plan. A repository or ticket label alone is insufficient. Create or update the ticket only within existing authorization; report missing scope before dependent implementation.

Read Link at session start, again before a material task, and before decisions that rely on project history. Resolve roots from environment overrides first (`MIXIO_LINK_ROOT`, then `LINK_ROOT`; `MIXIO_TRACKING_ROOT` for shared checks), then the user-local `~/.config/mixio/tracking.json` keys `link_root` and `tracking_root`. Maintainer setup supplies the actual checkout paths in that local JSON file; keep machine paths out of committed documentation. Validate the resolved directories and required files. Never substitute the current repository or an invented path when configuration is absent.

Use this safe JSON resolver; it reads data without sourcing shell configuration:

```sh
mixio_resolve_root() {
  python3 - "$1" <<'PYROOT'
import json, os, sys
from pathlib import Path
key = sys.argv[1]
env_names = {"link_root": ("MIXIO_LINK_ROOT", "LINK_ROOT"),
             "tracking_root": ("MIXIO_TRACKING_ROOT",)}
required = {"link_root": ("LINK.md", "link.py"),
            "tracking_root": ("scripts/tracking.py", "docs/tracking-cli.md",
                              "config/tracking-repositories.json")}
try:
    value = next((os.environ[name] for name in env_names[key]
                  if os.environ.get(name)), None)
    if value is None:
        config = json.loads((Path.home() / ".config/mixio/tracking.json").read_text())
        value = config.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError("configure the environment or user-local tracking.json")
    root = Path(value).expanduser()
    if not root.is_absolute() or not root.is_dir():
        raise ValueError("configured root must be an existing absolute directory")
    missing = [name for name in required[key] if not (root / name).is_file()]
    if missing:
        raise ValueError("configured root is missing: " + ", ".join(missing))
    print(root.resolve())
except (OSError, ValueError, TypeError, AttributeError, KeyError) as error:
    print(f"{key} unavailable: {error}", file=sys.stderr)
    sys.exit(1)
PYROOT
}
mixio_link_root="$(mixio_resolve_root link_root)" || { return 1 2>/dev/null || exit 1; }
mixio_link() {
  if command -v lnk >/dev/null 2>&1; then
    lnk "$@"
  else
    python3 "$mixio_link_root/link.py" "$@"
  fi
}
mixio_link health "$mixio_link_root"
mixio_link brief 'session start' "$mixio_link_root"
mixio_link query 'TICKET: task and repository scope' "$mixio_link_root" --budget micro
```

Use `lnk` when available and the Python entry point otherwise. Pass the verified root after positional text and before flags, as shown above.

Inspect relevant IMPs, scoped TBRs, and MPRs; include applicable MPR mitigations in acceptance criteria. Follow stronger existing repository Link checkpoints as well. The Python fallback executes the same CLI lifecycle; it does not replace a required CLI checkpoint with MCP.

**Done when:** live ticket scope and Link results, or explicit availability failures, are recorded with the working branch/worktree before code changes.

## 2. Material changes: keep execution and decisions current

Update the live ticket after scope changes, discoveries that change acceptance criteria, blockers, meaningful implementation or verification results, and handoffs. Include the affected repository and evidence; read back the update and state to confirm it persisted. Before contacting another person through a comment or message, ensure the session authorizes that communication. Plane owns committed work and its state; repository docs/specs own implementation details; Link contains source-backed synthesis and reviewed memory.

Record factual source notes with provenance, date, affected repository, and ticket while the context is available. Source notes and session captures may be written within authorized documentation work. Durable memory changes remain proposal-only until explicitly approved: use `propose-memories` or `session-end`, inspect candidates, and preserve the review gate. A lesson being useful does not authorize accepting a memory.

**Done when:** the ticket reflects the material change and readback succeeded; factual notes and any pending memory proposals are distinguishable.

## 3. Verify and hand off: report each lifecycle separately

Run the checks required by the acceptance criteria and repository instructions. In the ticket and handoff, report each dimension separately, with evidence or an explicit unknown:

- **Implemented:** the change exists in the named working tree, commit, or PR.
- **Verified:** named checks passed on the referenced change; record failures and skipped checks.
- **Merged:** a merge is confirmed by repository evidence.
- **Deployed:** the target environment and deployed revision are confirmed.

Do not infer merged or deployed from a passing test, an open PR, or an implementation claim. A Done state requires the ticket's actual completion contract, not merely coding progress. Read the live ticket again after updates and report unresolved blockers, pending review, and the next action. Capture concise factual session notes and proposal-only lessons through the Link lifecycle; perform graph maintenance only when relevant writes require it.

**Done when:** acceptance-criteria results, lifecycle evidence, final live ticket state, Link/source-note status, and remaining work are visible in the handoff.

## 4. Unavailable services and retry evidence

If Plane, Link, the shared registry, or an update/readback is unavailable, state which checkpoint failed and why. Save a local retry artifact outside the repository (for example in the user's private state directory) containing the ticket, repository, branch/worktree, timestamp, attempted operation, safe error summary, intended update, and exact retry action. Exclude credentials and sensitive response bodies. Mark that checkpoint **unsynced** until the retry and readback succeed. Continue independent work only when its scope is already authorized; missing acceptance criteria block dependent implementation. A local artifact, cache, or mirror is not evidence that Plane or Link was updated.

## Shared checks and ownership

Resolve `tracking_root` with the same setup function above. The verified shared checkout contains `scripts/tracking.py`, `docs/tracking-cli.md`, and `config/tracking-repositories.json`. Read that CLI document before coverage, mirror-drift, or PR checks and inspect `--help`. Pass local paths explicitly with `--checkout KEY=PATH`; the registry stores ownership, not developer filesystem paths. Missing or invalid configuration follows the unavailable/retry checkpoint above.

```sh
mixio_tracking_root="$(mixio_resolve_root tracking_root)" || { return 1 2>/dev/null || exit 1; }
python3 "$mixio_tracking_root/scripts/tracking.py" --help
```

Maintainers update the canonical protocol and ownership registry in the shared tracking repository, then propagate the versioned managed block and this exact protocol to maintained checkouts. Preserve existing agent instructions, dirty edits, and CLAUDE symlinks. Validate every accessible registered worktree; report inaccessible or newly created worktrees as coverage gaps. `studio/apps/mixio-lens` inherits Studio ownership and its root protocol. Public skill payloads must remain free of internal hosts, credentials, and mandatory private tracking setup.

Where installed, `.github/workflows/mixio-tracking.yml` checks protocol integrity and an explicit governing ticket line in the PR body without private credentials. Refresh its protocol hash when propagating protocol changes. This offline guard checks ticket syntax and registered ownership only; confirm ticket existence, acceptance criteria, and current state through the live Plane checkpoints above. Required branch protection and a successful hosted CI run require separate repository configuration and evidence.
