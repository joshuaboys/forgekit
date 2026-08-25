# Forgekit

A share-nothing Git host for the agent era. One static Rust binary.

Work on **cheap durable storage**. Review through **checkpoints** (commit + session context). **Promote** to GitHub when it is actually a release — not on every agent thrash.

```text
agent work  ──►  forgekit (local / store / r2)  ──►  checkpoint review  ──►  promote  ──►  GitHub
```

**Status:** MVP + Phase 2 starts — `memory`, `filesystem`, and `r2` backends; WAL + CAS tip; checkpoints; gated promote with optional **real GitHub evidence push**; JSON HTTP API; CLI with `init`.

---

## Install

**Requirements:** [Rust](https://rustup.rs/) 1.80 or newer (`rustc --version`).

### One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/joshuaboys/forgekit/main/install.sh | bash
```

Or:

```bash
cargo install --git https://github.com/joshuaboys/forgekit --locked --bin forgekit
```

That puts `forgekit` on your `PATH` (usually `~/.cargo/bin`).

### Build from a clone

```bash
git clone https://github.com/joshuaboys/forgekit.git
cd forgekit
cargo build --release -p forgekit-cli
# binary: ./target/release/forgekit
```

### Verify

```bash
forgekit --help
forgekit init
```

---

## Quick start (lowest friction)

```bash
forgekit init
# edit forgekit.toml if needed (defaults are filesystem + local)

forgekit serve &

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

forgekit checkpoint list --owner acme --name app
# approve the checkpoint id, then:
# forgekit promote --owner acme --name app --ref main --actor josh
```

### Optional: real GitHub promote push

1. Create a fine-grained or classic token with `contents: write` on the target repo.
2. In `forgekit.toml`:

```toml
[github]
repository = "you/your-repo"
push = true
token_env = "GITHUB_TOKEN"
required_checkpoint_kind = "release"
```

3. `export GITHUB_TOKEN=ghp_...`
4. Restart `forgekit serve`. Promote now pushes **evidence** (JSON of tip + checkpoint) onto `refs/heads/forgekit/promotes` via the Git Data API. The working branch is not rewritten.

### Optional: R2 / S3 cloud storage

```toml
backend = "r2"

[r2]
bucket = "my-forgekit"
endpoint = "https://<accountid>.r2.cloudflarestorage.com"
region = "auto"
access_key_env = "R2_ACCESS_KEY_ID"
secret_key_env = "R2_SECRET_ACCESS_KEY"
prefix = "prod/"   # optional key prefix
```

```bash
export R2_ACCESS_KEY_ID=...
export R2_SECRET_ACCESS_KEY=...
forgekit serve
```

CAS is process-local for R2 (single host per bucket/prefix). Multi-host coordination is a later work item.

---

## How to use it properly

Forgekit is not a drop-in replacement for `git push origin main` on every save. Treat it as the **day-to-day host** for noisy agent work; GitHub stays the **public / release** surface.

### Mental model

| Surface | Role |
| ------- | ---- |
| **Forgekit host** | Primary place agents write. Cheap, durable, concurrent-safe (WAL + CAS). |
| **Checkpoint** | The review moment: a commit bound to session context (prompt, tools, files). Status moves `pending` → `approved`. |
| **Promote** | Explicit “this tip is ready for the satellite.” Gated on an **approved** checkpoint of the required kind (default `release`). Optionally pushes evidence to GitHub. |
| **GitHub** | Where humans and CI expect history. Not the daily dump for agent churn. |

### Choose a backend

| Backend | Use when |
| ------- | -------- |
| `filesystem` | Real work on one machine. **Default from `forgekit init`.** |
| `r2` | Shared / durable cloud object store (R2, S3-compatible). |
| `memory` | Unit tests and throwaway demos only. |

Run **one** `forgekit serve` per data directory / bucket prefix. CLI commands are HTTP clients against `listen`.

### The intended loop

```text
1. init + serve   — long-running host
2. create repo    — once per project
3. virtual push   — agents write often; attach session + kind when you want a review unit
4. list / inspect — human or agent reviews the checkpoint
5. approve        — deliberate accept of that review unit
6. promote        — only after approve; records intent and optionally pushes evidence
```

### Agent skill

Point coding agents at [`.agents/skills/forgekit-operator/SKILL.md`](.agents/skills/forgekit-operator/SKILL.md) so they learn **when** to checkpoint and promote.

---

## CLI

```text
forgekit init [--path forgekit.toml] [--force]
forgekit serve --config <path>
forgekit status --config <path>
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
| `GET` | `/v1/status` | Mode, backend, repo count, `github_push` |
| `GET` | `/v1/repos` | List repos |
| `POST` | `/v1/repos` | Create repo `{"owner","name"}` |
| `GET` | `/v1/repos/{owner}/{name}` | Repo tip summary |
| `POST` | `/v1/repos/{owner}/{name}/push` | Virtual push (+ optional checkpoint) |
| `GET` | `/v1/repos/{owner}/{name}/checkpoints` | List checkpoints |
| `GET` | `/v1/repos/{owner}/{name}/checkpoints/{id}` | Inspect checkpoint |
| `POST` | `/v1/repos/{owner}/{name}/checkpoints/{id}/approve` | Approve `{"actor"}` |
| `POST` | `/v1/repos/{owner}/{name}/promote` | Gated promote (+ optional GitHub push) |
| `GET` | `/v1/repos/{owner}/{name}/events` | WAL events |

Gate errors: `409` conflict / not pending, `403` promote refused, `404` missing, `400` bad name.

---

## How it works (short)

- **WAL + CAS tip** — acknowledged write is visible on the next read; concurrent tip races produce one winner and `CasConflict` for the loser.
- **Checkpoint** — native object bound to a commit, with kind (`work` / `stable` / `release`), status, and session.
- **Promote** — requires an **approved** checkpoint of the configured kind on the tip. Records the intent; when `github.push = true`, also pushes evidence to `refs/heads/forgekit/promotes`.

Inspired by Continuity / [walgit](https://github.com/tobi/walgit) (WAL + CAS) and [Entire](https://entire.io) (checkpoint UX, MIT).

---

## Develop

```bash
cargo test --workspace
cargo run -p forgekit-cli -- init
cargo run -p forgekit-cli -- serve --config forgekit.toml
```

Planning lives under [`plans/`](plans/index.aps.md).

---

## Not in this window

- Smart HTTP (`git-upload-pack` / `git-receive-pack`)
- Full Git object history on GitHub (promote pushes **evidence**, not a mirror of every virtual commit)
- Multi-host CAS for R2 (single-host lock today)
- LFS, bundle-uri, OIDC

---

## Licence

[MIT](LICENSE). Third-party notices: [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).
