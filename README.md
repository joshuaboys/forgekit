# Forgekit

[![Rust](https://github.com/joshuaboys/forgekit/actions/workflows/rust.yml/badge.svg)](https://github.com/joshuaboys/forgekit/actions/workflows/rust.yml)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://rustup.rs/)

A share-nothing Git host for the agent era. One static Rust binary.

Work on **cheap durable storage**. Review through **checkpoints** (commit + session context). **Promote** to GitHub when it is actually a release — not on every agent thrash.

```text
agent work  ──►  forgekit (local / store / r2)  ──►  checkpoint review  ──►  promote  ──►  GitHub
```

**Status:** MVP + Phase 2 starts — `memory`, `filesystem`, and `r2` backends; WAL + CAS tip; checkpoints; gated promote with optional **real GitHub evidence push**; JSON HTTP API; a zero-config CLI covering the whole loop.

Pre-1.0 and moving. The HTTP API, config format, and on-disk layout can change between releases — pin a commit if you depend on it. See [Not in this window](#not-in-this-window) for what it deliberately does not do yet.

---

## Install

### One-liner — no Rust needed

```bash
curl -fsSL https://raw.githubusercontent.com/joshuaboys/forgekit/main/install.sh | bash
```

Downloads a static prebuilt binary (Linux / macOS, x86_64 / arm64) into `~/.local/bin`,
and falls back to building from source if your platform has no binary.

Knobs: `FORGEKIT_VERSION=v0.1.0`, `FORGEKIT_INSTALL_DIR=/usr/local/bin`, `FORGEKIT_FROM_SOURCE=1`.

### From source

Needs [Rust](https://rustup.rs/) 1.80 or newer.

```bash
cargo install --git https://github.com/joshuaboys/forgekit --locked --bin forgekit
```

Or from a clone:

```bash
git clone https://github.com/joshuaboys/forgekit.git
cd forgekit
cargo build --release -p forgekit-cli
# binary: ./target/release/forgekit
```

### Verify

```bash
forgekit --help
```

---

## Quick start

No config file and no `curl`. `forgekit serve` runs on built-in defaults
(`filesystem` backend, `./data`, `127.0.0.1:8088`) until you choose to write one.

```bash
forgekit serve &

forgekit repo create acme/app

forgekit push acme/app \
  -m "feat: auth" \
  --file src/auth.rs \
  --kind release \
  --prompt "add auth"

forgekit checkpoint list acme/app
forgekit checkpoint approve acme/app --id <checkpoint-id> --actor josh
forgekit promote acme/app
```

Promote before approval fails closed, on purpose:

```text
forgekit: 403 Forbidden: promote refused for acme/app: missing approved release checkpoint
```

That refusal **is** the gate. Approve the checkpoint rather than working around it.

### Optional: write a config

```bash
forgekit init          # writes forgekit.toml
```

Only needed to change the defaults — pick a different backend, listen address, or
wire up GitHub. Everything below is optional.

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
1. serve          — long-running host (init only if you need non-default config)
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

Repos are addressed as `owner/name`.

```text
forgekit init    [--path forgekit.toml] [--force]
forgekit serve   [--config <path>]
forgekit status  [--config <path>]

forgekit repo create <owner>/<name>
forgekit repo list

forgekit push <owner>/<name> -m <message> [--ref main] [--actor <who>]
                             [--file <path>]... [--kind work|stable|release]
                             [--prompt <session prompt>]

forgekit checkpoint list    <owner>/<name>
forgekit checkpoint inspect <owner>/<name> --id <id>
forgekit checkpoint approve <owner>/<name> --id <id> [--actor <who>]

forgekit promote <owner>/<name> [--ref main] [--actor <who>]
```

Every command accepts `--config <path>` (default `forgekit.toml`). When that file is
absent the built-in defaults are used, so the CLI works out of the box.

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

## Contributing

Issues and pull requests are welcome — especially bug reports with a reproducible
`forgekit.toml` and the failing request.

- `cargo test --workspace` must pass; CI runs it on every PR.
- [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md) is **generated** — do not hand-edit it. See
  [`tools/starters/acknowledgements/AGENTS.md`](tools/starters/acknowledgements/AGENTS.md);
  CI fails on drift.
- Planning and work items live under [`plans/`](plans/index.aps.md); the architecture note is in
  [`designs/`](designs/2026-08-24-forgekit-architecture.design.md).

## Licence

[MIT](LICENSE). Third-party notices: [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).
