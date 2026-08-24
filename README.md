# Forgekit

A share-nothing Git host for the agent era. One static Rust binary.

Work on **cheap durable storage**. Review through **checkpoints** (commit + session context). **Promote** to GitHub when it is actually a release — not on every agent thrash.

```text
agent work  ──►  forgekit (local / store)  ──►  checkpoint review  ──►  promote  ──►  GitHub
```

**Status:** MVP complete — `memory` and `filesystem` backends, WAL + CAS tip, checkpoints, gated promote, JSON HTTP API, and CLI.

> Promote currently **records** the transition in the WAL. A network `git push` to GitHub is Phase 2. Smart HTTP is also out of this MVP window; agents and tools talk JSON under `/v1/`.

---

## Install

**Requirements:** [Rust](https://rustup.rs/) 1.80 or newer (`rustc --version`).

### Option A — install the binary from GitHub

```bash
cargo install --git https://github.com/joshuaboys/forgekit --locked --bin forgekit
```

That puts `forgekit` on your `PATH` (usually `~/.cargo/bin`).

### Option B — build from a clone

```bash
git clone https://github.com/joshuaboys/forgekit.git
cd forgekit
cargo build --release -p forgekit-cli
```

Binary:

```text
./target/release/forgekit
```

Optional install into `~/.cargo/bin`:

```bash
cargo install --path crates/forgekit-cli --locked
```

### Verify

```bash
forgekit --help
```

---

## How to use it properly

Forgekit is not a drop-in replacement for `git push origin main` on every save. Treat it as the **day-to-day host** for noisy agent work; GitHub stays the **public / release** surface.

### Mental model

| Surface | Role |
| ------- | ---- |
| **Forgekit host** | Primary place agents write. Cheap, durable, concurrent-safe (WAL + CAS). |
| **Checkpoint** | The review moment: a commit bound to session context (prompt, tools, files). Status moves `pending` → `approved` (or rejected/superseded later). |
| **Promote** | Explicit “this tip is ready for the satellite.” Gated on an **approved** checkpoint of the required kind (default `release`). |
| **GitHub** | Where humans and CI expect history. Not the daily dump for agent churn. |

Do **not** promote every intermediate agent commit. Push freely on the cheap side; promote only when you mean release (or a stable handoff).

### Choose a backend

| Backend | Use when |
| ------- | -------- |
| `filesystem` | Real work. State lives under `data_dir` and survives restart. **Use this by default.** |
| `memory` | Unit tests and throwaway demos only. State dies with the process and is **not** shared with other CLI invocations. |

```toml
backend = "filesystem"
data_dir = "./data"
```

Run **one** `forgekit serve` per data directory. CLI commands (`status`, `checkpoint`, `promote`) are HTTP clients against `listen` — they need that serve process up.

### The intended loop

```text
1. serve          — long-running host
2. create repo    — once per project
3. virtual push   — agents write often; attach session + kind when you want a review unit
4. list / inspect — human or agent reviews the checkpoint
5. approve        — deliberate accept of that review unit
6. promote        — only after approve; records the release intent
```

#### 1. Start the host (leave it running)

```bash
cp forgekit.example.toml forgekit.toml
# set backend = "filesystem"
forgekit serve --config forgekit.toml
```

```bash
curl -s http://127.0.0.1:8088/healthz   # {"ok":true}
forgekit status --config forgekit.toml
```

#### 2. Create a repository once

```bash
curl -s -X POST http://127.0.0.1:8088/v1/repos \
  -H 'content-type: application/json' \
  -d '{"owner":"acme","name":"app"}'
```

Names are restricted to alphanumeric, `-`, `_`, `.` (max 64). Invalid names return `400`.

#### 3. Virtual-push work (often)

A push always advances the tip (WAL + CAS). A **checkpoint** is created only when you send `session` and/or `kind`:

```bash
curl -s -X POST http://127.0.0.1:8088/v1/repos/acme/app/push \
  -H 'content-type: application/json' \
  -d '{
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
  }'
```

| Field | Guidance |
| ----- | -------- |
| `ref` | Branch name (`main`) or full ref (`refs/heads/main`) |
| `actor` | Who wrote this (agent id or human) |
| `kind` | `work` (default if only session), `stable`, or `release`. Use **`release`** when this tip should be promotable. |
| `session` | Capture enough context to review later without re-running the agent. |
| omit both `session` and `kind` | Durable push **without** a checkpoint (fine for intermediate noise). |

Working refs always point at the **code commit**, not the checkpoint id. Checkpoint trailer form: `Entire-Checkpoint: <12-char-id>`.

Concurrent pushes race the tip: exactly one wins; the loser gets `409` / `CasConflict`. Retry after re-reading the tip.

#### 4. Review checkpoints

```bash
forgekit checkpoint list --config forgekit.toml --owner acme --name app
forgekit checkpoint inspect --config forgekit.toml --owner acme --name app --id <12-char-id>
```

Or HTTP:

```bash
curl -s http://127.0.0.1:8088/v1/repos/acme/app/checkpoints
curl -s http://127.0.0.1:8088/v1/repos/acme/app/events   # WAL: push, checkpoint, approve, promote
```

Read the session payload. That is the review surface — not a CI log.

#### 5. Approve when you mean it

```bash
curl -s -X POST http://127.0.0.1:8088/v1/repos/acme/app/checkpoints/<id>/approve \
  -H 'content-type: application/json' \
  -d '{"actor":"josh"}'
```

Only `pending` checkpoints can be approved. A second approve returns `409`. Approval is durable (WAL `approve`).

#### 6. Promote only after approval

```bash
forgekit promote --config forgekit.toml --owner acme --name app --ref main --actor josh
```

- **Without** an approved checkpoint of `github.required_checkpoint_kind` on that tip → **fails closed** (`403`).
- **With** a matching approved checkpoint → WAL `promote` entry is written (`remote=github`, `pushed=false`).

Until Phase 2, “promote” means **recorded release intent**, not a network push. You still ship to GitHub by whatever process you use today; the gate is the quality boundary inside Forgekit.

### Agent integration pattern

1. Keep `forgekit serve` running for the project (filesystem backend).
2. Agent finishes a unit of work → `POST .../push` with `kind` + `session` (prompt, tools, files touched).
3. Human or policy agent → `checkpoint list` / `inspect` → `approve` when the unit is acceptable.
4. Release operator → `promote` when the tip should leave the cheap primary.

Push **without** session/kind for high-churn intermediates so you do not flood the checkpoint list. Attach a checkpoint when the work is a coherent reviewable unit.

### What “done” looks like for a release tip

1. Tip commit exists on the ref you care about.
2. A checkpoint of kind `release` (or your configured kind) points at that commit.
3. That checkpoint is `approved`.
4. `promote` succeeds and appears in `/events`.

Anything short of that is still local agent work — which is the point.

---

## Quick start (copy-paste)

```bash
cp forgekit.example.toml forgekit.toml
# edit: backend = "filesystem"

forgekit serve --config forgekit.toml &

curl -s -X POST http://127.0.0.1:8088/v1/repos \
  -H 'content-type: application/json' \
  -d '{"owner":"acme","name":"app"}'

curl -s -X POST http://127.0.0.1:8088/v1/repos/acme/app/push \
  -H 'content-type: application/json' \
  -d '{
    "ref": "main",
    "message": "feat: auth",
    "actor": "pi",
    "files": ["src/auth.rs"],
    "kind": "release",
    "session": { "prompt": "add auth" }
  }'

forgekit checkpoint list --config forgekit.toml --owner acme --name app
# approve via HTTP, then:
forgekit promote --config forgekit.toml --owner acme --name app --ref main --actor josh
```

---

## CLI

```text
forgekit serve --config <path>          # start JSON HTTP host (long-running)
forgekit status --config <path>         # mode, backend, repo count
forgekit checkpoint list  --config <path> --owner <o> --name <n>
forgekit checkpoint inspect --config <path> --owner <o> --name <n> --id <id>
forgekit promote --config <path> --owner <o> --name <n> --ref <ref> [--actor <who>]
```

Default `--config` is `forgekit.toml` in the current directory. All commands except `serve` require a reachable host on `listen`.

---

## HTTP API (MVP)

Base: `http://<listen>` (example `http://127.0.0.1:8088`).

| Method | Path | Purpose |
| ------ | ---- | ------- |
| `GET` | `/healthz` | Liveness |
| `GET` | `/v1/status` | Mode, backend, repo count |
| `GET` | `/v1/repos` | List repos |
| `POST` | `/v1/repos` | Create repo `{"owner","name"}` |
| `GET` | `/v1/repos/{owner}/{name}` | Repo tip summary |
| `POST` | `/v1/repos/{owner}/{name}/push` | Virtual push (+ optional checkpoint) |
| `GET` | `/v1/repos/{owner}/{name}/checkpoints` | List checkpoints |
| `GET` | `/v1/repos/{owner}/{name}/checkpoints/{id}` | Inspect checkpoint |
| `POST` | `/v1/repos/{owner}/{name}/checkpoints/{id}/approve` | Approve `{"actor"}` |
| `POST` | `/v1/repos/{owner}/{name}/promote` | Gated promote |
| `GET` | `/v1/repos/{owner}/{name}/events` | WAL events |

Gate errors are intentional status codes, not 500s: `409` conflict / not pending, `403` promote refused, `404` missing, `400` bad name.

---

## How it works (short)

- **WAL + CAS tip** — acknowledged write is visible on the next read; concurrent tip races produce one winner and `CasConflict` for the loser.
- **Checkpoint** — native object bound to a commit, with kind (`work` / `stable` / `release`), status, and session. Optional Entire-compatible trailer: `Entire-Checkpoint: <12-char-id>`. Working refs stay on the code commit only.
- **Promote** — requires an **approved** checkpoint of the configured kind on the tip. Records the intent; does not yet push to GitHub.

Inspired by Continuity / [walgit](https://github.com/tobi/walgit) (WAL + CAS) and [Entire](https://entire.io) (checkpoint UX, MIT). Native storage is primary; Entire interop is optional emission.

---

## Develop

```bash
cargo test --workspace
cargo run -p forgekit-cli -- serve --config forgekit.example.toml
```

Planning lives under [`plans/`](plans/index.aps.md). Architecture: [`designs/2026-08-24-forgekit-architecture.design.md`](designs/2026-08-24-forgekit-architecture.design.md).

---

## Not in this MVP

- Smart HTTP (`git-upload-pack` / `git-receive-pack`)
- Real network push to GitHub on promote
- Cloud object backends (R2 / S3) — filesystem and memory only
- LFS, bundle-uri, OIDC

---

## Licence

[MIT](LICENSE). Third-party notices: [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).
