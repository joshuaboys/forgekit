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
echo "Installed. Quick start:"
echo "  forgekit init"
echo "  # optional: set github.repository + export GITHUB_TOKEN for promote push"
echo "  # optional: set backend = \"r2\" + R2_* env for cloud storage"
echo "  forgekit serve"
echo "  curl -s -X POST http://127.0.0.1:8088/v1/repos -H 'content-type: application/json' -d '{\"owner\":\"acme\",\"name\":\"app\"}'"
