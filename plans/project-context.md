# Project Context

> User-owned file. APS will never overwrite this.

## Overview

Forgekit is a single-binary Git host for humans and agents. Day-to-day work (especially high-churn agent work) lives on cheap durable storage. GitHub is the public / release surface. Quality is a **checkpoint** (review moment with agent session context), not a CI pipeline.

## Team

- @joshuaboys — owner

## Tech Stack

- Rust (edition 2021), single static binary
- WAL + CAS tip for consistency; object-store backends later (R2 first)
- HTTP via Axum; `gix` / upstream `git` where they are the right tool
- APS for planning; MIT license

## Conventions

- Specs describe intent. Work items authorise execution. Do not implement Draft items.
- Canonical statuses: Draft → Ready → In Progress → Complete (or Blocked)
- Only a human marks work items Ready
- Commits for tracked files carry `APS: ITEM-ID` trailers
- Prefer `gix` / Rust; shell out to `git` only where measured
- Entire (MIT) informs checkpoint UX and trailer compatibility; native storage wins

## Active Decisions

- **D-001:** Object-store (or local disk in `local` mode) is source of truth; instances are disposable caches.
- **D-002:** Checkpoints are the review moment. Traditional CI is an optional event consumer.
- **D-003:** Hybrid mode: cheap primary + promote selected history to GitHub.
- **D-004:** Entire-compatible trailer/shadow-branch is optional emission, not the primary store.
- **D-005:** Modes (`local` | `store` | `hybrid`) are config, not separate products.
