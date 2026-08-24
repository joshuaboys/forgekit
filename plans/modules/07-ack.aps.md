# Acknowledgements

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| ACK | @joshuaboys | medium | Complete: 2026-08-24 |

**Last reviewed:** 2026-08-24

## Purpose

Keep third-party licence notices accurate and regenerable using the EddaCraft acknowledgements starter.

## In Scope

- Kit adopted at `tools/starters/acknowledgements` (pinned release)
- Consumer config (`attribution.toml`, `licences.toml`, `ACKNOWLEDGEMENTS.md`)
- Regenerated rust block from `Cargo.lock` once the shipping crate exists
- CI `--check` for expander now; generator once crates exist

## Out of Scope

- Editing generator or driver scripts
- Shipping the kit inside the crate
- Node / Go / Python blocks

## Interfaces

**Depends on:**

- CLI — shipping `Cargo.toml` / `Cargo.lock`

**Exposes:**

- `ACKNOWLEDGEMENTS.md` — curated thanks + generated rust notices

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### ACK-001: Kit adopted and expander is drift-free — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** The acknowledgements starter is vendored and the allow-list expander is the single source of truth.
- **Expected Outcome:** `tools/starters/acknowledgements` is present; `expand-licences.sh --check` exits 0.
- **Validation:** `tools/starters/acknowledgements/expand-licences.sh --check`
- **Files:** `tools/starters/acknowledgements/`, `attribution.toml`, `licences.toml`, `about.toml`, `ACKNOWLEDGEMENTS.md`
- **Confidence:** high
- **Results:** expander `--check` is green in CI.

### ACK-002: Rust notices generate from the lockfile — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** The shipping binary's third-party licences appear between the rust markers and stay fresh in CI.
- **Expected Outcome:** `generate-acknowledgements.sh --check` exits 0 against a real `Cargo.lock`.
- **Validation:** `tools/starters/acknowledgements/generate-acknowledgements.sh --check`
- **Files:** `ACKNOWLEDGEMENTS.md`, `.github/workflows/acknowledgements.yml`
- **Dependencies:** CLI-001, ACK-001
- **Confidence:** medium
- **Results:** rust block generated from Cargo.lock; generator `--check` enabled in CI.
