---
name: add-tool
description: Add a new MCP tool to PCBServer (or extend an existing one), from the KiCad API check to tests, registry, doc counts and the shipped skill. Use when the task is to add, expose or extend a tool such as teardrops, deleting vias or zones, or zone priority.
---

Work through these in order. Stop and ask the owner where a step says so.

1. **Can KiCad do it?** For a PCB tool, find the message or field in
   `crates/konnect-ipc/proto/` (for example `DeleteItems`, `Zone.priority`,
   `PadTeardropSettings`). Note the "Since: 10.0.x" comment; the tool must refuse on an older
   KiCad. If the API does not offer it, stop and report the options instead of editing the board file.
2. **Is the IPC client ready?** Check `crates/konnect-ipc/src/client.rs` for an existing wrapper.
   Add one only if it is missing, with a unit test for the message it builds.
3. **Find the closest existing tool** in the target toolset file under
   `crates/konnect-core/src/tools/` and read it fully, including its tests. The new tool follows
   its shape: argument reading, document binding, commit, readback, structured errors.
4. **Plan.** Write down: tool name, toolset, arguments, what it refuses and with which error kind,
   what it returns, how it is tested. Get the owner's approval before writing code.
5. **Implement** the `tool!(...)` definition and the `handle_*` function as described in
   "Adding a New Tool" in `DEV.md` and in `docs/DEVELOPING_TOOLS.md`.
6. **Tests.** Unit tests for argument handling and refusals, a served-dispatch test, and an
   `#[ignore]`d live test if the behaviour depends on KiCad. Run the one test first, then the crate.
7. **Register.** Update `tool_count` in `crates/konnect-core/src/router/registry.rs`, then run
   `cargo xtask fix-doc-counts`.
8. **Teach the client.** Add the tool to the matching skill in `crates/konnect/assets/skills/`
   (for PCB tools `kicad-pcb`), in the same style as its neighbours.
9. **Gate.** `cargo fmt --all`, the clippy command from `CLAUDE.md`, and
   `cargo test --workspace --locked --lib --tests`.
10. **Live check.** Build the release binary, and ask the owner to open a copy of a board in KiCad.
    Call the tool through the `pcbserver-dev` MCP registration (see `CLAUDE.md`), then have the owner
    look at the result in KiCad. Report what was run and what was not.
11. **Review and hand over.** Run `/code-review`, fix what it finds, commit with `-s`, and open the
    pull request with the template in `.github/pull_request_template.md`. Do not merge.
