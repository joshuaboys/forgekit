# Core (WAL, CAS, Host)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| CORE | @joshuaboys | high | In Progress |

**Last reviewed:** 2026-08-24

## Purpose

Own repository identity, the WAL, the CAS tip manifest, refs, and the host operations that every other module uses.

## In Scope

- Repository names and keys
- Manifest (seq, refs, pack ids, checkpoint ids)
- Immutable WAL entries
- CAS update of the tip
- Host operations: create repo, virtual push, list/get
- Concurrent CAS loser behaviour

## Out of Scope

- Physical object-store backends (STORE)
- Checkpoint semantics beyond storing ids on the manifest (CKPT)
- HTTP (HTTP)
- GitHub (GHUB)

## Interfaces

**Depends on:**

- STORE — durable get/put/cas/list

**Exposes:**

- Host — create, list, get
- ObjectBackend trait
- Manifest types

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [x] Human marks CORE-001 Ready (2026-08-24)

## Work Items

### CORE-001: Repository and tip exist — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** A named repository can be created and its empty tip read back.
- **Expected Outcome:** Create is idempotent-fail on conflict; get returns seq 0 and no refs.
- **Validation:** `cargo test -p forgekit-core create -- --nocapture`
- **Files:** `crates/forgekit-core/`
- **Confidence:** high
- **Results:** `create_repo_returns_empty_tip`, `create_repo_conflict_is_idempotent_fail`, `create_repo_rejects_invalid_name` pass.

### CORE-002: Virtual push is durable and visible — Draft

- **Status:** Draft
- **Intent:** A virtual push writes an immutable entry and CAS-advances the tip so the next read sees the new ref.
- **Expected Outcome:** seq increments by one; named ref points at the new commit; WAL lists the push.
- **Validation:** `cargo test -p forgekit-core push -- --nocapture`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** CORE-001
- **Confidence:** high

### CORE-003: CAS conflict is observable — Draft

- **Status:** Draft
- **Intent:** Two writers racing the tip produce exactly one winner and a conflict error for the loser.
- **Expected Outcome:** Second CAS against a stale tip fails without clobbering the winner.
- **Validation:** `cargo test -p forgekit-core cas -- --nocapture`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** CORE-002
- **Confidence:** high
