#!/usr/bin/env bash
# Install the forgekit binary into ~/.cargo/bin
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust from https://rustup.rs/ first." >&2
  exit 1
fi

echo "→ cargo install --git https://github.com/joshuaboys/forgekit --locked --bin forgekit"
cargo install --git https://github.com/joshuaboys/forgekit --locked --bin forgekit

echo
echo "Installed. Next:"
echo "  forgekit init"
echo "  forgekit serve"
echo "  # set GITHUB_TOKEN and github.repository in forgekit.toml to push promotes"
