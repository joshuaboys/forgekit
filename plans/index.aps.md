# Forgekit MVP

| Field   | Value |
| ------- | ----- |
| Status  | Draft |
| Owner   | @joshuaboys |
| Created | 2026-08-24 |

## Problem

Agent-heavy Git work is too expensive and too noisy on GitHub, and CI is the wrong review primitive for continuous agent loops. We need a single-binary host: cheap durable primary, checkpoints as the review moment, GitHub only when we mean to release.

## Success Criteria

- [x] A virtual push appends WAL and CAS-updates the tip; the next read sees it
- [x] A checkpoint can be created, inspected, and approved
- [ ] One binary serves a host with `local` and in-memory/filesystem `store` backends
- [ ] Promote is refused without the required approved checkpoint, then recorded
- [ ] JSON HTTP API and CLI expose status, repos, checkpoints, promote, events
- [ ] Core WAL / checkpoint / promote tests pass

## Constraints

- Single static Rust binary, MIT
- No database / leader / node identity
- Specs authorise outcomes, not tutorials
- Do not implement Draft work items
- Smart HTTP, LFS, bundle-uri, OIDC, real R2/S3 are out of this MVP window

## Designs

- [Forgekit Architecture](../designs/2026-08-24-forgekit-architecture.design.md)

## Modules

| Module | Purpose | Status | Dependencies |
| ------ | ------- | ------ | ------------ |
| [CORE](./modules/01-core.aps.md) | WAL, CAS tip, refs, host | Complete: 2026-08-24 | — |
| [STORE](./modules/02-store.aps.md) | Backend trait + memory/filesystem | In Progress | CORE |
| [CKPT](./modules/03-checkpoints.aps.md) | Checkpoints as the review moment | Complete: 2026-08-24 | CORE, STORE |
| [HTTP](./modules/04-http.aps.md) | JSON HTTP API | Draft | CORE, STORE, CKPT |
| [GHUB](./modules/05-github.aps.md) | Import record + gated promote | Draft | CORE, CKPT |
| [CLI](./modules/06-cli.aps.md) | `forgekit` binary surface | Draft | HTTP, GHUB |
| [ACK](./modules/07-ack.aps.md) | Third-party notices via acknowledgements-starter | Draft | CLI |

## Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Scope creeps into full Git protocol | MVP never ships | JSON + virtual push first; smart HTTP later |
| Checkpoints become a second Git | History pollution | Native objects off the working ref; optional trailer only |
| Promote-without-push confuses users | False sense of release | CLI/API make the record vs GitHub-push boundary explicit |
| CAS races under-tested | Silent divergence | Concurrent CAS tests in CORE |

## Open Questions

- [ ] First cloud backend after filesystem: R2 or generic S3?
- [ ] Promote gate: tip commit only, or ancestor walk?
- [ ] Session payload size limits?

## Decisions

- **D-001:** Object store (or local disk in `local`) is source of truth — _accepted_ (see project-context)
- **D-002:** Checkpoints are the review moment; CI is optional — _accepted_
- **D-003:** Hybrid cheap-primary + GitHub satellite — _accepted_
- **D-004:** Entire compatibility is emission, not primary store — _accepted_
- **D-005:** JSON API before smart HTTP — _accepted_ (design)

## What's Next

1. GHUB-001 promote gate
2. HTTP-001 health and status
3. CLI-001 serve

