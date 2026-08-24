# Forgekit Architecture

| Field   | Value |
| ------- | ----- |
| Status  | Draft |
| Created | 2026-08-24 |
| Modules | [CORE](../plans/modules/01-core.aps.md), [STORE](../plans/modules/02-store.aps.md), [CKPT](../plans/modules/03-checkpoints.aps.md), [HTTP](../plans/modules/04-http.aps.md), [GHUB](../plans/modules/05-github.aps.md), [CLI](../plans/modules/06-cli.aps.md) |

## Problem

Hosting Git for agent-scale work on GitHub is expensive (storage, bandwidth, LFS, Actions minutes) and the wrong quality primitive. Agents work continuously; push → pipeline → green/red fights that loop.

walgit / Continuity already show a share-nothing object-store Git host (WAL + CAS, disposable caches). That solves scale. It does not solve:

- explicit operational modes (laptop vs bucket vs hybrid)
- first-class agent review moments
- GitHub as a release satellite rather than the daily host

## Constraints

- Single static Rust binary
- No database, no leader, no node identity
- Consistency is never eventual: acknowledged write is visible on the next read
- Main working history stays clean (checkpoint context must not pollute it)
- Do not become a full forge (no issues / PRs / packages as core)
- Do not become a CI platform
- Entire is MIT — reuse concepts and trailer convention; do not take their storage as ours

## Design

### Shape

```
binary
  modes: local | store | hybrid
  protocol (JSON now; smart HTTP later)
  checkpoints + session capture
  events (webhooks / socket later)
        │
  core: refs + WAL + CAS tip + checkpoint objects
        │
  backend: memory | filesystem | R2 | S3-compatible | azure | gcs
        │
  promote ──► GitHub (public / release)
```

### Modes

| Mode | Primary | Consistency | GitHub |
| ---- | ------- | ----------- | ------ |
| `local` | disk | local locking / journal | optional promote |
| `store` | object store | WAL + CAS | optional promote |
| `hybrid` | local or store | as above | first-class promote |

### WAL + CAS

Per repository:

- Immutable content-addressed objects (packs / blobs / checkpoint bodies)
- Append-only WAL (`push`, `checkpoint`, `approve`, `promote`, `compact`)
- One CAS'd tip manifest (`seq`, refs, pack set, checkpoint ids)

A push is acknowledged only after objects are durable **and** the tip CAS succeeds. Instances cache; a conditional read of the tip is enough to know currency.

### Checkpoints (review moment)

Continuous session capture while the agent works. On commit / virtual push, a checkpoint is created:

- Points at a commit OID
- Carries session (prompt, transcript, tools, files, tokens, attribution)
- Kind: `work` | `stable` | `release`
- Status: `pending` | `approved` | `rejected` | `superseded`

Primary store: native objects beside the WAL. Optional Entire emission: commit trailer `Entire-Checkpoint: <id>` and later a shadow branch.

**Promote** (or a protected-ref update) may require an approved checkpoint of a declared kind. That is the gate. Traditional runners may subscribe to events; they are not the quality center.

### Hybrid / GitHub

- Import: clone GitHub → cheap backend, record the remote
- Daily: pushes and agent work stay cheap-side
- Promote: select refs/tags (optional history filter), enforce checkpoint gate, push to GitHub (Phase 2), record WAL `promote`

### HTTP surface (MVP)

JSON under `/v1/` for status, repos, virtual push, checkpoints, approve, promote, events. Smart HTTP, LFS, bundle-uri come after the core is true.

## Alternatives Considered

| Alternative | Pros | Cons | Verdict |
| ----------- | ---- | ---- | ------- |
| Adopt walgit as-is | Proven WAL+CAS | No modes, no checkpoints, no GitHub satellite | Keep the consistency idea; do not fork their product |
| Entire shadow branch as primary store | Existing CLI/UI | Tied to an upstream Git host; weak gates | Interop only |
| Zero-server remote helper | Simplest | Weak multi-writer, no host/events | Not the product |
| Traditional CI as the gate | Familiar YAML | Wrong for agents | Optional consumer |

## Implementation Notes

Order: CORE (WAL/CAS/host) → STORE (memory/fs) → CKPT → HTTP JSON → CLI → GHUB record. Real GitHub push, smart HTTP, and object-store backends are later modules/items, not MVP blockers for the consistency and checkpoint story.

Work items stay Draft until a human marks them Ready.

## Decisions

- **D-001:** WAL + CAS tip is the consistency primitive in `store` / `hybrid`.
- **D-002:** Checkpoints are first-class native objects; Entire format is optional emission.
- **D-003:** Promote is a gated WAL transition; actual GitHub push can lag the record.
- **D-004:** JSON API before smart HTTP.
- **D-005:** One binary, modes by config.

## Open Questions

- [ ] Which object-store backend is first after filesystem (R2 vs generic S3)?
- [ ] Ancestor walk vs tip-commit-only for promote gates in v1?
- [ ] How much session payload is stored vs referenced?
