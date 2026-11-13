# Vendored KiCad IPC API definitions

Every `.proto` file in this directory is an unmodified copy of KiCad's
`api/proto/` directory at one release tag. Nothing here is edited by hand; a
change to the API surface is a resync to a newer tag.

| | |
|---|---|
| Upstream | https://gitlab.com/kicad/code/kicad, directory `api/proto/` |
| Tag | `10.0.8` |
| Commit | `e432beac2668303194ba2040274f9e5efe773fc0` (tag object `7aca893cfa151bbf2f04375b686d1b905d583e56`) |
| Commit date | 2026-10-29 |
| Synced | 2026-11-13, all 15 files byte for byte |

The previous snapshot was KiCad 10.0.6 for all twelve files then present.

## What is compiled

`build.rs` compiles every file except `schematic/schematic_commands.proto`,
which at this tag declares the `kiapi.schematic.types` package and no messages.

## Licences

The files keep their upstream headers. All are GPL-3.0-or-later except
`common/types/variants.proto` and `common/commands/variant_commands.proto`,
which carry the MIT licence text. See `THIRD_PARTY.md` at the repository root.

## Refreshing

Replace the whole tree from one tag, never single files, so the snapshot stays
coherent (`board_types.proto` imports `embedded_files.proto`, for example):

```bash
TAG=10.0.8
cd crates/konnect-ipc/proto
for path in $(curl -s "https://gitlab.com/api/v4/projects/kicad%2Fcode%2Fkicad/repository/tree?path=api/proto&ref=$TAG&recursive=true&per_page=100" \
    | python3 -c 'import json,sys; [print(e["path"]) for e in json.load(sys.stdin) if e["type"]=="blob"]'); do
  rel=${path#api/proto/}; mkdir -p "$(dirname "$rel")"
  curl -sf "https://gitlab.com/kicad/code/kicad/-/raw/$TAG/$path" -o "$rel"
done
git ls-remote https://gitlab.com/kicad/code/kicad.git "refs/tags/$TAG^{}"   # commit for the table above
```

Then add any new file to `build.rs`, update the table above, and decide for
every new or changed field whether `builders.rs`, `client.rs` and `types.rs`
need to change (see `docs/KICAD_INTEGRATION.md`, "Protocol version").
