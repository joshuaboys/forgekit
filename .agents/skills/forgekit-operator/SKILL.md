---
name: forgekit-operator
description: >-
  How and when to use a running Forgekit host: virtual push, checkpoints as
  the review moment, approve, and gated promote. Use when agents write to
  Forgekit, review agent work, or decide whether a tip is ready to promote.
---

# Forgekit Operator

Forgekit is the **cheap primary host** for agent work. GitHub is the **release
satellite**. You do not promote every intermediate commit.

```text
work  →  virtual push (often)  →  checkpoint when reviewable
                                →  approve deliberately
                                →  promote only for release intent
```

## Prerequisites

- A long-running `forgekit serve --config <toml>` is up.
- Config uses `backend = "filesystem"` for real work (`memory` is tests only).
- CLI commands talk HTTP to `listen` in that config. If serve is down, stop.

Base URL: `http://<listen>` from the config (example `http://127.0.0.1:8088`).

## When to do what

| Situation | Action |
| --------- | ------ |
| New project on this host | `POST /v1/repos` once |
| Intermediate agent edits, not a coherent unit | Virtual push **without** `session` or `kind` |
| A coherent unit ready for human/policy review | Virtual push **with** `kind` + `session` |
| Tip should be eligible for release promote | Use `kind: "release"` (or the host's configured required kind) |
| Reviewing agent output | `checkpoint list` / `inspect` — read the session, not CI logs |
| Accepting a review unit | `POST .../checkpoints/{id}/approve` |
| Declaring the tip a release candidate | `promote` **only after** matching approve |
| Promote fails with 403 | Missing approved checkpoint of required kind on tip — do not force |

## Virtual push

```http
POST /v1/repos/{owner}/{name}/push
Content-Type: application/json
```

```json
{
  "ref": "main",
  "message": "feat: auth",
  "actor": "pi",
  "files": ["src/auth.rs"],
  "kind": "release",
  "session": {
    "prompt": "add auth middleware",
    "tools": ["edit", "test"],
    "attribution": "pi"
  }
}
```

Rules:

- **Omit** both `session` and `kind` for high-churn intermediate pushes so the
  checkpoint list stays reviewable.
- **Include** them when the work is a single reviewable unit.
- `kind`: `work` | `stable` | `release`. Default kind when only session is set
  is `work`. Promote gate defaults to requiring **`release`**.
- Working refs always point at the **code commit**, never the checkpoint id.
- On `409` / CAS conflict: re-read the tip, rebuild the push, retry once.

## Checkpoints

- List: `GET /v1/repos/{owner}/{name}/checkpoints`
- Inspect: `GET /v1/repos/{owner}/{name}/checkpoints/{id}`
- CLI: `forgekit checkpoint list|inspect --config <toml> --owner … --name …`

A checkpoint is pending until approved. Trailer form:
`Entire-Checkpoint: <12-char-id>`.

Approve only when the session and diff match what you intended:

```http
POST /v1/repos/{owner}/{name}/checkpoints/{id}/approve
Content-Type: application/json

{"actor": "josh"}
```

Second approve on a non-pending checkpoint → `409`. Do not invent workarounds.

## Promote

```text
forgekit promote --config <toml> --owner <o> --name <n> --ref main --actor <who>
```

or `POST /v1/repos/{owner}/{name}/promote` with body:

```json
{
  "ref": "main",
  "actor": "josh",
  "required_kind": "release",
  "remote": "github"
}
```

Rules:

- **Fail closed** without an approved checkpoint of `required_kind` on that tip.
- Success records a WAL `promote` entry (`pushed=false` in this MVP).
- Do **not** treat promote as “run CI then ship.” CI is optional; the checkpoint
  is the quality gate.
- Network push to GitHub is **not** performed by this host yet. Recording
  promote is still required before you treat the tip as release-ready inside
  Forgekit.

## Errors (do not map to “retry until 200”)

| Status | Meaning | Agent response |
| ------ | ------- | -------------- |
| `400` | Invalid name | Fix owner/name; do not retry |
| `403` | Promote refused | Create/approve the right checkpoint first |
| `404` | Missing repo or checkpoint | Create or use the correct id |
| `409` | Exists / CAS conflict / not pending | Re-read state; for CAS, rebase intent on current tip |

## What this skill does not do

- Replace `git` smart HTTP (not in MVP).
- Auto-approve or auto-promote.
- Require a checkpoint on every push.
- Talk to GitHub’s network on promote.
- Plan product work (use APS skills for that).

## Minimal happy path

1. Ensure `forgekit serve` is running (filesystem backend).
2. Create repo once if needed.
3. Push intermediate work without checkpoints as needed.
4. When the unit is reviewable: push with `kind: "release"` and a real `session`.
5. Inspect checkpoint → approve with a real actor.
6. Promote only then.
