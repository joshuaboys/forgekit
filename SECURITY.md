# Security policy

## Reporting a vulnerability

Report privately through
[GitHub Security Advisories](https://github.com/joshuaboys/forgekit/security/advisories/new).
Please do not open a public issue for a vulnerability.

Include what you did, what happened, and the config or request that triggered it.
A reproduction against a local `forgekit serve` is ideal.

Forgekit is pre-1.0 and maintained on a best-effort basis. There is no support
window for older tags: fixes land on `main` and go out in the next release.

## Threat model — read this before exposing a host

**The HTTP API is unauthenticated.** There are no tokens, sessions, or access
control on any endpoint. Anyone who can reach the `listen` address can:

- create repos and push arbitrary content,
- **approve checkpoints** — the gate that authorises a release, and
- **trigger a promote**, which pushes to GitHub using your configured token.

The default `listen = "127.0.0.1:8088"` is what keeps that private. Treat the
listen address as the entire security boundary:

- Keep it on loopback, or on a network only you and your agents can reach.
- Do not bind `0.0.0.0` on a shared or public host.
- Put it behind an authenticating reverse proxy, a private network, or an SSH
  tunnel if it must be reachable from elsewhere.

Approval is a *workflow* gate, not an *authorisation* one. It records intent and
stops accidental promotion; it does not defend against someone who can already
reach the API.

## Credentials

Secrets are read from environment variables and are never written to
`forgekit.toml` — the config only stores the *name* of the variable
(`token_env`, `access_key_env`, `secret_key_env`).

- `forgekit.toml` is gitignored by default. Keep it that way.
- The GitHub token needs only `contents: write` on the target repo. Prefer a
  fine-grained token scoped to that one repo.
- R2/S3 keys should be scoped to the single bucket and prefix in use.
- Anyone who can reach the API can cause the host to use these credentials, even
  though they cannot read them back. See the threat model above.

## What promote publishes

`promote` with `github.push = true` writes an evidence commit to
`refs/heads/forgekit/promotes` containing the repo owner/name, tip oid,
checkpoint id, actor, and message. **If the target GitHub repo is public, that
evidence is public.** Do not put secrets in commit messages, session prompts, or
checkpoint payloads.
