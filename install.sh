#!/usr/bin/env bash
# Install the forgekit binary.
#
#   curl -fsSL https://raw.githubusercontent.com/joshuaboys/forgekit/main/install.sh | bash
#
# Downloads a prebuilt static binary when one exists for this platform, and
# falls back to building from source with cargo. No Rust needed for the
# prebuilt path.
#
# Env:
#   FORGEKIT_VERSION      tag to install (default: latest)
#   FORGEKIT_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   FORGEKIT_FROM_SOURCE  set to 1 to skip the download and use cargo
set -euo pipefail

REPO="joshuaboys/forgekit"
VERSION="${FORGEKIT_VERSION:-latest}"
INSTALL_DIR="${FORGEKIT_INSTALL_DIR:-$HOME/.local/bin}"

say()  { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

build_from_source() {
  command -v cargo >/dev/null 2>&1 || die \
    "no prebuilt binary for this platform and cargo not found. Install Rust from https://rustup.rs/ and re-run."
  say "→ building from source with cargo"
  cargo install --git "https://github.com/${REPO}" --locked --bin forgekit
  say ""
  say "Installed to ~/.cargo/bin/forgekit"
  quickstart
  exit 0
}

quickstart() {
  cat <<'TXT'

Quick start (no config file needed):

  forgekit serve &
  forgekit repo create acme/app
  forgekit push acme/app -m "feat: auth" --file src/auth.rs --kind release --prompt "add auth"
  forgekit checkpoint list acme/app
  forgekit checkpoint approve acme/app --id <id> --actor you
  forgekit promote acme/app

TXT
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$arch" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) return 1 ;;
  esac
  case "$os" in
    Linux)  printf '%s-unknown-linux-musl' "$arch" ;;
    Darwin) printf '%s-apple-darwin' "$arch" ;;
    *) return 1 ;;
  esac
}

[ "${FORGEKIT_FROM_SOURCE:-0}" = "1" ] && build_from_source

command -v curl >/dev/null 2>&1 || die "curl is required"

TARGET="$(detect_target)" || {
  warn "unsupported platform: $(uname -s) $(uname -m)"
  build_from_source
}

if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/forgekit-${TARGET}.tar.gz"
else
  URL="https://github.com/${REPO}/releases/download/${VERSION}/forgekit-${TARGET}.tar.gz"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say "→ downloading forgekit-${TARGET} (${VERSION})"
if ! curl -fsSL "$URL" -o "$TMP/forgekit.tar.gz"; then
  warn "no prebuilt binary at ${URL}"
  build_from_source
fi

tar -xzf "$TMP/forgekit.tar.gz" -C "$TMP"
[ -f "$TMP/forgekit" ] || die "archive did not contain a forgekit binary"

mkdir -p "$INSTALL_DIR"
install -m 755 "$TMP/forgekit" "$INSTALL_DIR/forgekit"
say "Installed ${INSTALL_DIR}/forgekit"

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *) warn ""
     warn "note: ${INSTALL_DIR} is not on your PATH. Add this to your shell profile:"
     warn "  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac

quickstart
