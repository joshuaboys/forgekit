# GitHub Hybrid (Promote)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| GHUB | @joshuaboys | high | In Progress |

**Last reviewed:** 2026-08-24

## Purpose

Treat GitHub as the public/release satellite: record a remote, and promote selected refs only when the checkpoint gate passes.

## In Scope

- Configured GitHub remote record
- Promote as a WAL transition
- Optional required checkpoint kind (`release` by default)
- Explicit “recorded, not pushed” boundary for MVP

## Out of Scope

- Actual `git push` to GitHub (Phase 2)
- History rewriting / agent-noise filters (Phase 2)
- Continuous branch mirror
- GitHub Checks mirroring

## Interfaces

**Depends on:**

- CORE — WAL / refs
- CKPT — approved checkpoint of required kind

**Exposes:**

- Promote operation (host + later HTTP/CLI)

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### GHUB-001: Promote is refused without the gate — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** Promote of a tip without an approved checkpoint of the required kind fails.
- **Expected Outcome:** Error names the missing kind; no WAL promote entry is written.
- **Validation:** `cargo test -p forgekit-core promote`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** CKPT-002
- **Confidence:** high
- **Results:** `promote_is_refused_without_the_gate` pass.

### GHUB-002: Promote is recorded after approval — Draft

- **Status:** Draft
- **Intent:** After approval, promote appends a WAL promote entry targeting GitHub.
- **Expected Outcome:** WAL kind is promote; events include promote; GitHub network push is not required.
- **Validation:** `cargo test -p forgekit-core promote`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** GHUB-001
- **Confidence:** high
