# Forgekit

A share-nothing Git host for the agent era. One static Rust binary.

Work on cheap storage. Review through checkpoints. Promote to GitHub when it is actually a release.

**Status:** planning. Implementation starts when APS work items are marked Ready.

- Intent: [plans/index.aps.md](plans/index.aps.md)
- Architecture: [designs/2026-08-24-forgekit-architecture.design.md](designs/2026-08-24-forgekit-architecture.design.md)
- Context: [plans/project-context.md](plans/project-context.md)

## Idea

GitHub stays the public / release surface. Day-to-day and agent work run against a cheap primary (`local` disk or object store). A **checkpoint** is the review moment — commit plus session context — not a CI pipeline. Promote is a gated transition.

Inspired by Continuity / walgit for WAL+CAS, and by Entire (MIT) for checkpoint UX. Native storage; optional Entire trailer compatibility.

## License

MIT (to be added with the first implementation crate).
