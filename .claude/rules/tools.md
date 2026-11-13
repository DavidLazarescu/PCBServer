---
paths:
  - "crates/*/src/tools/**"
  - "crates/*/src/router/**"
---

# Working on tools

- `docs/RELIABILITY_CONTRACT.md` is binding. Reuse the existing structured errors, document
  binding, transaction and readback helpers in `tools/mod.rs` instead of writing new ones.
- The tool description and input schema are the only things the AI client sees. Write the
  description for a model that has never read this code: what the tool does, what it needs
  (KiCad open or closed, board saved), what it refuses, what it returns.
- Input records are closed: every accepted field is declared in `properties`
  (see "Adding a New Tool" in `DEV.md`).
- A PCB write goes through KiCad IPC inside one commit, then reads the result back and compares.
  A mismatch is an error, not a warning.
- A tool that can only work on a saved file while KiCad holds the board open refuses; it does not
  edit the file underneath the editor.
- Every new tool gets unit tests, and a served-dispatch test for the typo/no-write case. A live
  test against KiCad is `#[ignore]`d and documented in `docs/TESTING_AND_RELEASE.md`.
- After adding or removing a tool: registry `tool_count`, `cargo xtask fix-doc-counts`, and the
  matching skill under `crates/konnect/assets/skills/` so clients learn the tool exists.
