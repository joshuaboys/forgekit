# Forgekit

A share-nothing Git host for the agent era. One static Rust binary.

Work on cheap storage. Review through checkpoints. Promote to GitHub when it is actually a release.

**Status:** MVP complete (`local` / memory / filesystem, checkpoints, gated promote, JSON HTTP, CLI).

```text
forgekit serve --config forgekit.example.toml
forgekit status --config forgekit.example.toml
forgekit checkpoint list --owner acme --name app
forgekit promote --owner acme --name app --ref main
```

- Intent: [plans/index.aps.md](plans/index.aps.md)
- Architecture: [designs/2026-08-24-forgekit-architecture.design.md](designs/2026-08-24-forgekit-architecture.design.md)
- Context: [plans/project-context.md](plans/project-context.md)
- Acknowledgements: [ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md) ([kit](https://github.com/eddacraft/acknowledgements-starter))

## Idea

GitHub stays the public / release surface. Day-to-day and agent work run against a cheap primary (`local` disk or object store). A **checkpoint** is the review moment — commit plus session context — not a CI pipeline. Promote is a gated transition.

Inspired by Continuity / walgit for WAL+CAS, and by Entire (MIT) for checkpoint UX. Native storage; optional Entire trailer compatibility.

## License

[MIT](LICENSE)
