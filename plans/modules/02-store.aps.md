# Store (Backends)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| STORE | @joshuaboys | high | Complete: 2026-08-24 |

**Last reviewed:** 2026-08-24

## Purpose

Give the host a durable object backend with get, put, CAS, and prefix list — without caring which medium it is.

## In Scope

- Backend trait
- In-memory backend (tests / ephemeral)
- Filesystem backend (`local` / cache)
- Keys as opaque strings

## Out of Scope

- R2 / S3 / Azure / GCS (later work)
- Compaction policy
- Cache eviction

## Interfaces

**Depends on:**

- CORE — key layout conventions

**Exposes:**

- Backend — get, put, cas, list_prefix

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### STORE-001: Memory backend satisfies the contract — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** An in-memory backend implements get/put/cas/list for tests and `memory` mode.
- **Expected Outcome:** CAS succeeds only when expected matches; list_prefix returns stored keys.
- **Validation:** `cargo test -p forgekit-store memory`
- **Files:** `crates/forgekit-store/`
- **Confidence:** high
- **Results:** `memory_cas_and_list_prefix` and `memory_hosts_create` pass.

### STORE-002: Filesystem backend persists across process restart — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** A filesystem backend stores the same contract on disk for `local` / cache.
- **Expected Outcome:** Values written in one process are readable after reopen; CAS still exclusive.
- **Validation:** `cargo test -p forgekit-store filesystem`
- **Files:** `crates/forgekit-store/`
- **Dependencies:** STORE-001
- **Confidence:** high
- **Results:** `filesystem_persists_across_reopen`, `filesystem_cas_is_exclusive`, `filesystem_hosts_create_and_survives_reopen` pass.
