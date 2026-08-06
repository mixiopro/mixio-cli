#!/bin/sh
# Short-URL wrapper for `curl -fsSL .../install.sh | bash`. Forwards to the
# real installer cargo-dist generates and attaches to every release, so this
# file never needs updating and there's one source of truth for install logic.
set -e
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mixiopro/mixio-cli/releases/latest/download/mixio-cli-installer.sh | sh
