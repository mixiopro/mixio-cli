# Short-URL wrapper for `irm .../install.ps1 | iex`. Forwards to the real
# installer cargo-dist generates and attaches to every release, so this file
# never needs updating and there's one source of truth for install logic.
irm https://github.com/mixiopro/mixio-cli/releases/latest/download/mixio-cli-installer.ps1 | iex
