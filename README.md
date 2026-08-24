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

## Quick start

### 1. Config

```bash
cp forgekit.example.toml forgekit.toml   # forgekit.toml is gitignored
```

Example (`forgekit.example.toml`):

```toml
listen = "127.0.0.1:8088"
mode = "local"
backend = "memory"          # or "filesystem"
data_dir = "./data"         # used when backend = "filesystem"

[github]
remote = "github"
required_checkpoint_kind = "release"
```

| Field | Meaning |
| ----- | ------- |
| `listen` | HTTP bind address |
| `mode` | Operational mode label (`local` / `store` / `hybrid`) — config, not separate products |
| `backend` | `memory` (ephemeral) or `filesystem` (durable under `data_dir`) |
| `github.required_checkpoint_kind` | Kind required to pass the promote gate (default `release`) |

**Tip:** Use `backend = "filesystem"` for anything you care about keeping. `memory` is for tests and throwaway demos; it does not share state across processes.

### 2. Start the host

```bash
forgekit serve --config forgekit.toml
```

Health check:

```bash
curl -s http://127.0.0.1:8088/healthz
# {"ok":true}
```

### 3. Create a repo and virtual-push (with a checkpoint)

```bash
# create
curl -s -X POST http://127.0.0.1:8088/v1/repos \
  -H 'content-type: application/json' \
  -d '{"owner":"acme","name":"app"}'

# push + pending checkpoint (kind release, with a session)
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
```

### 4. Inspect, approve, promote

CLI (talks to the running host via `listen` in the config):

```bash
forgekit status --config forgekit.toml
forgekit checkpoint list --config forgekit.toml --owner acme --name app
forgekit checkpoint inspect --config forgekit.toml --owner acme --name app --id <12-char-id>

# approve is HTTP for now
curl -s -X POST http://127.0.0.1:8088/v1/repos/acme/app/checkpoints/<id>/approve \
  -H 'content-type: application/json' \
  -d '{"actor":"josh"}'

forgekit promote --config forgekit.toml --owner acme --name app --ref main --actor josh
```

Promote **without** an approved checkpoint of the required kind fails closed (`403`). After approval, promote appends a WAL `promote` entry (`pushed=false` until Phase 2).

---

## CLI

```text
forgekit serve --config <path>          # start JSON HTTP host
forgekit status --config <path>         # mode, backend, repo count
forgekit checkpoint list  --config <path> --owner <o> --name <n>
forgekit checkpoint inspect --config <path> --owner <o> --name <n> --id <id>
forgekit promote --config <path> --owner <o> --name <n> --ref <ref> [--actor <who>]
```

Default `--config` is `forgekit.toml` in the current directory.

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
