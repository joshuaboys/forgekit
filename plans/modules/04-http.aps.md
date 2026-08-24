# HTTP (JSON API)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| HTTP | @joshuaboys | high | Complete: 2026-08-24 |

**Last reviewed:** 2026-08-24

## Purpose

Expose host operations over versioned HTTP so humans, agents, and a later UI can drive the binary without speaking Git yet.

## In Scope

- Listen address from config
- `/healthz`
- `/v1/status`, repos CRUD-lite, virtual push, checkpoints, approve, events
- JSON errors that preserve CAS / gate failures

## Out of Scope

- Smart HTTP (`git-upload-pack` / `git-receive-pack`)
- LFS
- Bundle-uri
- OIDC / token auth (auth mode `none` for MVP)
- Web UI inside the binary

## Interfaces

**Depends on:**

- CORE, STORE, CKPT — operations to serve
- GHUB — promote endpoint once GHUB-002 exists (may stub 501 until then)

**Exposes:**

- HTTP JSON `/v1/*`

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### HTTP-001: Status and health are served — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** A running binary answers liveness and host status.
- **Expected Outcome:** `/healthz` is ok; `/v1/status` reports mode, backend, repo count.
- **Validation:** `cargo test -p forgekit-server health`
- **Files:** `crates/forgekit-server/`
- **Dependencies:** CORE-001, STORE-001
- **Confidence:** high
- **Results:** `healthz_is_ok` and `health_status_reports_mode_backend_and_repo_count` pass.

### HTTP-002: Repo and checkpoint API match host operations — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** HTTP can create a repo, virtual-push, list checkpoints, and approve.
- **Expected Outcome:** JSON round-trip equals in-process host behaviour; gate errors are 409/403 not 500.
- **Validation:** `cargo test -p forgekit-server api`
- **Files:** `crates/forgekit-server/`
- **Dependencies:** HTTP-001, CKPT-002
- **Confidence:** medium
- **Results:** `api_create_push_checkpoint_approve_roundtrip` and `api_gate_errors_are_not_500` pass.
