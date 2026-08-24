# Checkpoints (Review Moment)

| ID | Owner | Priority | Status |
| --- | ----- | -------- | ------ |
| CKPT | @joshuaboys | high | Complete: 2026-08-24 |

**Last reviewed:** 2026-08-24

## Purpose

Make agent (and human) work reviewable: a checkpoint is the durable unit that binds a commit to session context, and can be approved as a gate.

## In Scope

- Checkpoint object (id, commit, kind, status, sessions, files)
- Create on push when session/kind present
- List / inspect
- Approve
- Optional Entire trailer string on the object
- Native storage in the backend (not a working-branch commit)

## Out of Scope

- Entire shadow-branch emission (later)
- Agent hook installers
- Traditional CI runners
- Ancestor-walk policy beyond tip (open question)

## Interfaces

**Depends on:**

- CORE — host, WAL, manifest checkpoint ids
- STORE — object durability

**Exposes:**

- Checkpoint create / list / get / approve
- Trailer helper for Entire-compatible id

## Ready Checklist

Change status to **Ready** when:

- [x] Purpose and scope are clear
- [x] Dependencies identified
- [x] At least one work item defined
- [ ] Human marks module Ready

## Work Items

### CKPT-001: Checkpoint is created with the push — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** A virtual push that includes a session records a pending checkpoint linked to the new commit.
- **Expected Outcome:** Checkpoint is inspectable; trailer is `Entire-Checkpoint: <12-char-id>`; working refs stay the code commit only.
- **Validation:** `cargo test -p forgekit-core checkpoint`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** CORE-002
- **Confidence:** high
- **Results:** `checkpoint_is_created_with_the_push` and `checkpoint_trailer_format` pass.

### CKPT-002: Checkpoint can be approved — Complete: 2026-08-24

- **Status:** Complete: 2026-08-24
- **Intent:** An actor can approve a pending checkpoint and that fact is durable.
- **Expected Outcome:** Status becomes approved with actor and time; WAL records the approval.
- **Validation:** `cargo test -p forgekit-core approve`
- **Files:** `crates/forgekit-core/`
- **Dependencies:** CKPT-001
- **Confidence:** high
- **Results:** `approve_pending_checkpoint_is_durable` and `approve_missing_checkpoint_fails` pass.
