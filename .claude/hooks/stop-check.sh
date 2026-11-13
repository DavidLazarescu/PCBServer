#!/usr/bin/env bash
# Stop hook: when Rust sources or Cargo files have uncommitted changes, require
# rustfmt and `cargo check` to pass before Claude may finish. Clippy is left to CI:
# the system Rust here is newer than the pinned 1.96.0 and its clippy already fails
# on unchanged code (chunks_exact_to_as_chunks in konnect-sexp). Exit 2 blocks the stop and
# sends stderr back to Claude. Does nothing when no such file is changed.
set -u
cd "${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel)}" || exit 0

changed=$(git status --porcelain -- '*.rs' '*Cargo.toml' 'Cargo.lock' 2>/dev/null)
[ -z "$changed" ] && exit 0

if ! out=$(cargo fmt --all -- --check 2>&1); then
  {
    echo "stop-check: cargo fmt --all -- --check failed. Run 'cargo fmt --all'."
    echo "$out" | head -40
  } >&2
  exit 2
fi

if ! out=$(cargo check --workspace --locked --all-targets 2>&1); then
  {
    echo "stop-check: cargo check failed. Fix the errors below."
    echo "$out" | grep -v '^\s*\(Compiling\|Checking\|Finished\|Blocking\)' | tail -80
  } >&2
  exit 2
fi
exit 0
