# CLI (Binary)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| CLI | @joshuaboys | medium | Draft |

**Last reviewed:** 2026-08-24

## Purpose

Ship one `forgekit` binary people and agents actually run: config, serve, status, checkpoint inspect, promote.

## In Scope

- `forgekit serve --config`
- `forgekit status`
- `forgekit checkpoint list|inspect`
- `forgekit promote`
- TOML config + example file
- `memory` and `filesystem` backends selectable

## Out of Scope

- Installer / credential helper
- TUI
- Agent hook installer
- Multi-process roles beyond a single serve

## Interfaces

**Depends on:**

- HTTP — serve
- GHUB — promote command
- CKPT — checkpoint commands

**Exposes:**

- `forgekit` CLI

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### CLI-001: Serve starts from example config — Draft

- **Status:** Draft
- **Intent:** `forgekit serve` with the example TOML listens and answers health.
- **Expected Outcome:** Process stays up; `/healthz` succeeds against the configured listen address.
- **Validation:** `cargo test -p forgekit-cli serve`
- **Files:** `crates/forgekit-cli/`, `forgekit.example.toml`
- **Dependencies:** HTTP-001
- **Confidence:** medium

### CLI-002: Checkpoint and promote commands reach the host — Draft

- **Status:** Draft
- **Intent:** Operators can list/inspect checkpoints and request promote from the CLI.
- **Expected Outcome:** Commands fail closed on missing gate; succeed after approval.
- **Validation:** `cargo test -p forgekit-cli promote`
- **Files:** `crates/forgekit-cli/`
- **Dependencies:** CLI-001, GHUB-002, HTTP-002
- **Confidence:** medium
