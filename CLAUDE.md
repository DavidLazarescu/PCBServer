# PCBServer

Fork of Konnect (see `NOTICE`), an MCP server that lets AI assistants drive KiCad 10.
Rust workspace; the code still uses the upstream name `konnect` everywhere until the rename lands.

## Where things are written down

Read these instead of guessing; do not copy their content into this file.

- `DEV.md`: architecture, crate layout, build requirements, test suite.
- `docs/RELIABILITY_CONTRACT.md`: what every tool must guarantee. Read before writing or reviewing a tool.
- `docs/DEVELOPING_TOOLS.md`, `docs/TOOL_SYSTEM.md`: how tools and toolsets are built.
- `docs/KICAD_INTEGRATION.md`: IPC (live board) versus file editing (schematic).
- `docs/TESTING_AND_RELEASE.md`: test levels, live-KiCad tests, release steps.
- `crates/konnect-ipc/proto/`: the KiCad API definitions. What KiCad can do over IPC is decided here.

## Commands

```bash
cargo check --workspace                                         # fast compile check
cargo test -p konnect-core --lib <test_name>                    # one test
cargo test --workspace --locked --lib --tests                   # all tests
cargo clippy --workspace --locked --all-targets -- -D warnings  # lint gate, same as CI
cargo fmt --all                                                 # format
cargo xtask fix-doc-counts                                      # after adding or removing a tool
cargo build --release -p konnect                                # the server binary: target/release/konnect
```

A Stop hook (`.claude/hooks/stop-check.sh`) runs `cargo fmt --check` and `cargo check` when Rust
files are changed and blocks finishing until both pass. It does not run clippy (see Gotchas), so
run the clippy command yourself before a pull request and read its output with that in mind.

## Rules

- Never push upstream tags (`v*`) and never create a tag unasked: `release.yml` publishes on tag push.
- No release before the rename away from "konnect" is complete.
- License is AGPL-3.0-only. No code from Konnect-EDA or any closed source. New dependencies must
  pass `cargo deny check licenses`. Keep `NOTICE` and `THIRD_PARTY.md` intact.
- Commit with `git commit -s` (DCO, see `CONTRIBUTING.md`). One branch and one pull request per change;
  merging is the owner's decision.
- This is a private project. Nothing from the owner's employer goes into this repository: no board
  files, no names, no internal paths.
- Never test against a real project. Live tests use a copy of a board, or the fixtures under
  `crates/*/tests/fixtures/`.
- A tool refuses with a structured error rather than guessing, and reads back what it wrote.
  Match the surrounding code's level of care; do not add a tool that is weaker than its neighbours.

## Gotchas

- `rust-toolchain.toml` pins Rust 1.96.0, but this machine has no rustup and builds with the
  system Rust (1.99). Its clippy reports `chunks_exact_to_as_chunks` in
  `crates/konnect-sexp/src/transaction.rs` on unchanged code (checked 2026-11-13); that lint does
  not exist in 1.96.0. New findings beyond that one are ours. CI on 1.96.0 is the reference.
- PCB tools need KiCad running with the board open (IPC socket `/tmp/kicad/api.sock`).
  Schematic tools edit files and refuse while the sheet is open in the editor.
- The installed Konnect 0.13.0 (`~/.local/share/kicad/10.0/3rdparty/plugins/com_github_mixelpixx_konnect/`)
  is the owner's working tool. Do not replace or reconfigure it. To try a local build, register it
  under its own name: `claude mcp add pcbserver-dev -- "$PWD/target/release/konnect"`.
- The skills and agents the server installs for its users live in `crates/konnect/assets/`. They are
  product, not project tooling. Project tooling for developing PCBServer lives in `.claude/`.
- Tool counts are quoted in about seven files; the `doc_tool_counts` test fails if they are stale.
  Use `cargo xtask fix-doc-counts`, never edit the numbers by hand.

## How a change runs

1. Read the affected area and the docs above. For a PCB feature, confirm in
   `crates/konnect-ipc/proto/` that KiCad 10 offers it.
2. Write a short plan (what, where, how it is tested, what it refuses) and get the owner's approval.
3. Implement with tests on a branch. Adding a tool: use the `add-tool` skill.
4. Run `/code-review` on the diff before opening the pull request.
5. When the owner corrects something or the code surprises us, add one line to this file or to
   `.claude/rules/`.
