# Feature gaps and improvement backlog

An overview of everything known to be missing, broken or worth improving in PCBServer, to decide what to build next.
It consolidates four research reports and the project lead's own checks.
It is a backlog, not a plan: nothing here is scheduled.

## 0. How to read this document

**Date:** 2026-11-13. Repository state: `4f53f13` (fork of upstream Konnect at `676efce`). Installed KiCad: 10.0.8.

**Sources.**

| Code | Report | Method |
|---|---|---|
| R1 | KiCad 10 IPC API against the PCB tools | Read the vendored protos and the client and tool code. Nothing run. |
| R2 | Schematic, libraries, project files, kicad-cli | Read tool schemas and selected handlers; `kicad-cli --help` for every subcommand; token counts in stock libraries. |
| R3 | Upstream issues, PRs, roadmap, rejected items | All 201 open issues, 61 open PRs, wontfix issues, via `gh` (read only). |
| R4 | Other AI-for-PCB tools and designer complaints | Web research. Pages were read through a summarising fetch tool. |
| Lead | Project lead's own checks | KiCad C++ source (branch 10.0 and master), vendored protos, local branches. |

**Verification status** (one per item, sometimes two where sources agree):

| Status | Meaning |
|---|---|
| `source-verified` | Read in KiCad's C++ source. |
| `proto-only` | In the API definition. Whether KiCad's handler implements it was not checked. |
| `code-read` | Found by reading this repository's code. Not run. |
| `reported` | Stated in an upstream issue or PR. Not re-checked here. |
| `external` | From a web source. The link is given. Vendor claims stay vendor claims. |
| `unverified` | Inferred or assumed. Check before building. |

**Size scale** (rough, for comparison only):

| Size | Meaning |
|---|---|
| S | A wrapper, builder or parser already exists, or a small file edit. Hours to about a day. |
| M | Needs a new client wrapper, builder, parser or check. A few days. |
| L | Needs new machinery, an external integration, or a design decision first. A week or more. |

**How codes:**

| Code | Meaning |
|---|---|
| IPC | KiCad's live API, with the board open in the PCB editor. |
| file | S-expression or JSON file editing, with the editor closed or the lock guard in place. |
| cli | `kicad-cli`. |
| compute | Pure computation inside the server over data it already reads. |
| ext | An external tool or service run as a subprocess or API call. Licence noted where known. |
| n/a | Packaging, docs, CI or server internals. |

**Other conventions.**
- "min KiCad" is the oldest KiCad version that can serve the item. "any" means the item does not depend on KiCad. "11" means it needs KiCad 11 (master today).
- "untagged" in R1's tables means the message was already in the first vendored snapshot; its exact introduction version is unknown. "6-snap" means it first appears in the 10.0.6 snapshot.
- PR numbers are open upstream PRs unless marked "closed PR". `[R]` marks a PR labelled `status:ready-to-merge` upstream. `[D]` marks a draft. No other open PR carries a readiness label. Upstream is archived, so every PR has to be ported by hand (section 5).
- Short paths: `T/` is `crates/konnect-core/src/tools/`, `SE/` is `crates/konnect-schematic-editor/src/schematic/`, protos are under `crates/konnect-ipc/proto/`.
- Item IDs are stable labels for discussion (`P1-07`). A cross-reference such as "see P2-07" means the item lives there and is not repeated.

## 1. Findings that change the plan

1. **The vendored API definitions are stale.** Four board files were resynced from KiCad 10.0.6 (commit `3b4b961`). The rest are from the initial import (`dd49a86`), tag not recorded. Teardrop settings on pads and vias (`PadTeardropSettings`) exist only from 10.0.7 (lead, seen in KiCad's 10.0 branch). The installed KiCad is 10.0.8. So a proto resync to 10.0.8 (P1-01) comes before a teardrop tool (P1-02). R1's "the API has no teardrop fields" describes the stale snapshot. Upstream #616 and #500 ask for the same resync.
2. **Correction to upstream #500.** #500 says the vendored proto lacks `Via.parent`, so a via re-encode would lose data. In this fork the field is present (`board/board_types.proto:499`, field 7, code-read), brought in by the 10.0.6 resync. Via tools (P1-03 to P1-06) are not blocked by it.
3. **No live schematic editing on KiCad 10.** KiCad 10.0's schematic editor registers only `GetOpenDocuments` plus the shared commit, create, update, delete and hit-test handlers. It has no `RevertDocument`, no `RunAction`, and no item types worth using (source-verified, `eeschema/api/api_handler_sch.cpp`, branch 10.0). KiCad master has a full schematic API: revert, items, selection, variants, ERC tasks, place symbol from library. Live schematic editing is a KiCad 11 topic. On KiCad 10, file editing stays the schematic path.
4. **Live netclasses are possible on KiCad 10.** `GetNetClasses` and `SetNetClasses` are served by KiCad 10.0 (source-verified, `common/api/api_handler_common.cpp`). Live netclass editing is in progress on branch `feat/netclasses-over-ipc` (P1-45). Net-to-class assignment patterns are not in the API, so they stay a file edit (P7-02).
5. **Protection against concurrent human edits has a design.** A separate investigation is finished. It recommends a per-item revision token, checked before every live PCB write, plus a server-side cache of what each item looked like when last read. A write against a changed item fails with a new `stale_read` error. Items that touch this (P0-15, P0-16, P0-17, P0-39, P0-84) should follow that design. The investigation also found a defect: `modify_trace` creates new copper when the segment it should replace is already gone (P0-84, code-read).
6. **Contradictions to resolve by testing against a live KiCad before building.**

   | Topic | One side | Other side | Item |
   |---|---|---|---|
   | Active layer | R1: proto has `GetActiveLayer` / `SetActiveLayer` (`board_commands.proto:365-375`) | Upstream #610: no IPC for it, by design | P1-40 |
   | Per-zone refill | R1: `RefillZones.zones` takes a KIID list (`board_commands.proto:232-238`) | Upstream `API_MIGRATIONS.md`: not available | P1-14 |
   | Title block write | R1: `SetTitleBlockInfo` first seen in the 10.0.6 snapshot | R3 (#714): KiCad 10.0.1+ | P1-38 |
   | Footprint field update | R1: `Field{text, visible}` is in the API | #836: `UpdateItems` on a footprint `Field` returns OK and changes nothing | P1-18 |

7. **61 open upstream PRs can no longer land upstream.** Before porting any of them, the licensing basis is checked once. They were submitted to an AGPL-3.0 repository whose contribution terms kept contributions available under AGPL-3.0. Authorship is preserved in the commits.
8. **Missing primitives made one MCP server worse than plain file editing.** An external test (R4, ProductOfAmerica, author of mcp-server-kicad) found plain Claude Code got 18 of 18 single edits right and the MCP plugin 3 of 18 ([source](https://github.com/ProductOfAmerica/mcp-server-kicad), external, single report). Causes: missing primitives (pad net, DNP flag, swapping one footprint), direct edits forbidden outright, and a sync that rewrote most footprints. Two of those primitives are missing here: pad net (P1-27) and DNP (P1-16 on the board, P1-57 in the schematic). The single-footprint swap is unclear (P1-24).

## 2. Priority tiers

The tiers are the lead's recommendation. Within a tier, order is not a ranking.

### P0. Correctness

**Why first.** These tools return success while producing a wrong result: shorted nets, dropped data, boards KiCad refuses to load, or descriptions that promise what the tool does not do. Every new feature built on top of them inherits the error. Many already have an upstream fix waiting. Merge-order dependencies among those fixes: #901 before #899 and #900; #782 before #781 and #857; #917 before #913, #914, #915, #916 and #920; #850 before #849; #820 before #824 and #969; #965 before #966; #906 before #902 and #905; #909 before #907, #908 and #926.

#### P0, high severity (R3 section 2 "High")

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P0-01 | Wire routing shorts nets | `connect_pins` / `batch_connect_pins` L-route joins any pin tip it passes over and reports success. | file | 10.0 | M | #887; fix PR #901 | reported |
| P0-02 | Net stubs join foreign pins | `connect_to_net` stubs join any pin they reach. | file | 10.0 | S | #899; after PR #901 | reported |
| P0-03 | Junctions dotted on unrelated T's | `add_wire`, `batch_add_wire`, `connect_to_net` dot every junction-less T on the sheet, joining unrelated nets. | file | 10.0 | S | #900; after PR #901 | reported |
| P0-04 | Duplicate reference wires the first | With two symbols sharing a reference, `connect_*` wires the first and reports success. | file | 10.0 | S | #781; after PR #782 | reported |
| P0-05 | Raw-text edit of first `(at ` | `move_labels_by_offset`, `rotate_schematic_label`, `bulk_move_schematic_components` rewrite a name containing `(at`. | file | 10.0 | S | #803, #813; PR #814, then #813 | reported |
| P0-06 | Private fields lost | KiCad-private fields are read as a field named "private"; a whole-file write drops their values. | file | 10.0 | S | #924; PR #931 | reported |
| P0-07 | Sheet identity goes stale | Sheets stay `stale_target`: `top_level_sheets` UUID mismatch, nested re-add, `edit_sheet new_file`, foreign project names, multi-root refused. | file | 10.0 | M | #913-#916, #920, #478; PR #917 base; fork draft dylbertram/Konnect#1; R2 sheet row | reported |
| P0-08 | `copy_routing_pattern` copies nothing | Always copies 0 on KiCad-saved boards; it text-matches a two-space indent. | file | 10.0 | S | #802; PR #809 [R] | reported |
| P0-09 | DSN double rotation | DSN export rotates pads of rotated footprints twice. | file | 10.0 | S | #840; PR #847 | reported |
| P0-10 | DSN ignores per-layer padstacks | KiCad 10 per-layer padstacks are exported wrongly. Fix refuses them. | file | 10.0 | S | #842; PR #848 | reported |
| P0-11 | Flip does not mirror inner layers | Closed-board flip and back-side library refresh keep In2 as In2 instead of In5. | file | 10.0 | S | #831; PR #864 | reported |
| P0-12 | Refresh puts pads on missing layer | Front-side library refresh puts pads on an inner layer the board does not have. | file / IPC | 10.0 | S | #863; no PR | reported |
| P0-13 | Unknown layer crashes KiCad | An unknown layer is sent as `BL_UNDEFINED`; KiCad 10.0.5 crashes on it. | IPC | 10.0 | S | #844, #237; PR #850 | reported |
| P0-14 | File fallback writes layer aliases | Fallbacks write `F.Silkscreen` style aliases; KiCad refuses to load the board. | file | 10.0 | S | #849; after PR #850 | reported |
| P0-15 | Fallbacks overwrite newer boards | Seven board file fallbacks overwrite a board changed between read and write. | file | 10.0 | M | #845; PR #852 | reported |
| P0-16 | `rename_project` overwrites saves | Overwrites saves made in between. | file | 10.0 | S | #851; PR #868 | reported |
| P0-17 | Project-file writes reverted or unguarded | Eeschema's save silently reverts `.kicad_pro` writes. Only pcbnew is checked; `~X.kicad_pro.lck` is ignored. No revision check. | file | 10.0 | M | #804, #964; PR #817; R2 K5: `T/pcb_routing.rs:1329`, `konnect-sexp/src/writer.rs:261-283` | reported, code-read |
| P0-18 | Copper zone marks every net routed | So no pad-net change can be synced on real boards. | IPC | 10.0 | S | #779; PR #862 | reported |
| P0-19 | Board sync output incomplete | No library fields or attributes, no symbol fields (LCSC, MPN); drops groups, points, variants, Exclude-from-BOM parts, pad fabrication properties. | IPC | 10.0 (pad fab property: 11) | L | #789, #788, #836, #875, #959; PRs #820, #824 [D], #865, #956 | reported |
| P0-20 | Placement silently re-nets a pad | `set_component_placements` lets KiCad move a pad onto another net's copper. DRC passes; the short reaches Gerbers. | IPC | 10.0 | M | #826; needs scope review | reported |
| P0-21 | SVG logo geometry wrong | `import_svg_logo` ignores SVG transforms and fills compound-path holes solid. | compute | any | S | #957, #958 | reported |
| P0-22 | Out-of-range design rules reported as success | `set_design_rules` writes minima outside KiCad's load range; KiCad resets them. | file | 10.0 | S | #963; partly PR #965 | reported |
| P0-23 | IPC-D-356 export unreachable | `export_netlist format:"ipc"` runs `sch export netlist` on the board. `cli::export_ipcd356()` exists, never called. | cli | 10.0 | S | #968; R2 D1: `T/pcb_export.rs:612-623`, `cli.rs:1409-1440, 1791` | reported, code-read |
| P0-24 | Decoupling audit by pin name | `audit_decoupling` classifies power pins by name only, flipping the review verdict. | compute | any | M | #903; design questions open | reported |
| P0-25 | `create_symbol` duplicates names | Appends a symbol whose name already exists. | file | 10.0 | S | #908; PR #909 | reported |
| P0-26 | `allow_soldermask_bridges` stripped | `set_footprint_metadata` strips it and cannot set it. 220 stock footprints use it. | file | 10.0 | S | #902; after PR #906; R2 D5: `T/footprint_metadata.rs:12-20` | reported, code-read |
| P0-27 | KiCad 11 footprints give false pass | `(transform ...)` footprints are unparsed; `score_placement` scores 0, skips 24, passes. | file | 11 | M | #680; design-needed | reported |
| P0-28 | Unknown price or stock read as 0 | JLCPCB catalogue ranks unknown-price parts as cheapest. | compute | any | S | #582 | reported |
| P0-29 | Schematic writes hang | Writes hang until the MCP timeout (two reporters). | file | 10.0 | M | #405; needs information | reported |
| P0-30 | macOS sync preflight fails | `GetOpenDocuments` answers `AS_UNHANDLED` when the Project Manager owns `api.sock`. Likely KiCad side. | IPC | 10.0.6 | M | #780; needs information | reported |
| P0-31 | Valid root-prefixed paths rejected | Sync rejects valid root-prefixed instance paths. | IPC | 10.0 | S | #723; closed PR #725 (rebuild needed) | reported |
| P0-32 | Templates electrically wrong | `stm32_minimal` cannot be applied and is wrong; `ldo_3v3` contradicts its own note. | file | 10.0 | S | #783; PR #867 | reported |

#### P0, defects found by reading this repository (R2 D1 to D8, K5, K6)

D1 is P0-23, D5 is P0-26, K5 is part of P0-17.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P0-33 | Variants fixture differs from KiCad | Fixture puts `"variants"` at top level of `.kicad_pro`; a project file saved by KiCad 10.0.8 has `schematic.variants`. | file | 10.0 | S | R2 D2; `crates/konnect-sexp/tests/fixtures/variants/` (README says a GUI re-save is pending) | code-read |
| P0-34 | Old pin-number syntax | `create_symbol` writes `(pin_numbers hide)`; KiCad 10 stock uses `(pin_numbers (hide yes))`. Acceptance not tested. | file | 10.0 | S | R2 D3; `T/library.rs:4459`; opt-in test `KONNECT_TEST_SYMBOL_CLI` | code-read |
| P0-35 | Old file version stamps | Footprints written as `20240108` (stock: `20260206`); schematic scaffold `20250610` (KiCad 10: `20260306`). Consistency only. | file | 10.0 | S | R2 D4; `T/library.rs:848`; `konnect-sexp/src/schematic.rs:25` | code-read |
| P0-36 | `group_components` is not a group | Writes a custom property, not a KiCad `(group ...)`. Real groups: P1-72. | file | 10.0 | S | R2 D6; `tool-directory.md:108` | code-read |
| P0-37 | Generic delete without UUIDs | `batch_delete` deletes any item by UUID, but nothing lists text, graphics, bus entries, no-connects or label UUIDs. Power pseudo-labels carry the symbol UUID. | file | 10.0 | S | R2 D7: `T/sch_batch.rs:904-921`; #800; #954 (PR #955, description). Fix: P1-59 | code-read, reported |
| P0-38 | Bus aliases ignored | `schematic.bus_aliases` in `.kicad_pro` is not read, so bus connectivity analysis ignores alias names. | file | 10.0 | M | R2 D8; `T/sch_bus.rs`, `T/sch_connectivity.rs` | code-read |
| P0-39 | `.kicad_dru` written without guard | `set_layer_constraints` upserts rules with no open-editor check. Board Setup overwrites the file. | file | 10.0 | S | R2 K6: `T/verification.rs:190-205`, handler near line 610 | code-read |

#### P0, found by the concurrent-edit investigation

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P0-84 | `modify_trace` adds copper for a vanished segment | When the segment to replace is already gone (for example deleted by a human), it still creates the new copper instead of refusing. | IPC | 10.0 | S | Concurrent-edit investigation; `pcb_routing.rs:1244-1290`, `client.rs:2403`. Fix with the `stale_read` design (section 1, finding 5) | code-read |

#### P0, stale claims and misleading descriptions

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P0-40 | `set_active_layer` refusal text | Says the protocol exposes no mutation; the proto has `SetActiveLayer`. Test first (P1-40). | n/a | 10.0 | S | R1; `pcb_board.rs:1151, 2021`; `API_MIGRATIONS.md:1035`; #610 | proto-only |
| P0-41 | `delete_graphics` reference-image text | Says reference images cannot be identified; `ReferenceImage.id` exists since the 10.0.6 sync. | n/a | 10.0.6 | S | R1; `pcb_board.rs:1190`; `board_types.proto:991-1008` | code-read |
| P0-42 | `refill_zones` per-zone text | Says per-zone selection is unavailable; client always sends an empty list. Test first (P1-14). | n/a | 10.0 | S | R1; `client.rs:2434-2442` | proto-only |
| P0-43 | `set_board_size` says "appends" | Its IPC path replaces plain-segment outlines. | n/a | 10.0 | S | R1; `pcb_board.rs:1380-1398` | code-read |
| P0-44 | Bottom-side text not mirrored | `add_board_text` always sends `mirror=false`, even on `B.SilkS`. Thickness differs between IPC and file paths. | IPC / file | 10.0 | S | R1: `pcb_board.rs:2494`; #457 | code-read, reported |
| P0-45 | `edit_component` promises more | Description says "value or other properties"; it only rewrites the value. | n/a | 10.0 | S | R1; `pcb_components.rs:2669-2693` | code-read |
| P0-46 | Reliability inventory stale | Lists live/file board readers (#542), which is closed; the ratchet can drop from 3 to 2. | n/a | any | S | R3; `docs/reliability-legacy-inventory.json`; #542 | reported |

#### P0, lower-severity defects (R3 section 2 "Medium" and "Low")

Defects placed in other tiers: #659 (P2-20), #517 and #536 (P8-08), #764 and #870 (P8-14), #131 (P8-11), #793 (P4-03), #786 (P4-04), #964 (P0-17), #800 (P0-37), #457 (P0-44), #537 (P8-13), #420 (P8-08), #670 and #841 (P6-03).

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P0-47 | Schematic diff churn | Writer's `(pts ...)` wrapping differs from eeschema; about 360 of 3712 lines change per edit. | file | 10.0 | M | #210; #691, #692 (PRs #693, #694) | reported |
| P0-48 | Nested sheet page number | A nested sheet gets its parent's page number. | file | 10.0 | S | #515 | reported |
| P0-49 | Orphan schematic | `create_schematic` makes a sheet no project owns. | file | 10.0 | S | #660 | reported |
| P0-50 | Field rotation undocumented | Field rotation is symbol-relative and not documented. | n/a | 10.0 | S | #890 | reported |
| P0-51 | Overlap check gaps | `check_schematic_overlaps` ignores label and field text; power symbol at a pin tip reported as overlap. | compute | any | S | #889; #745 (PR #815) | reported |
| P0-52 | Missing junctions | On a moved pin, in `connect_passthrough`, and where three wire ends meet. | file | 10.0 | S | #935, #936; #897 (PR #937) | reported |
| P0-53 | Net queries incomplete | `get_net_connections` returns no pins; an unknown net name gets an empty success. | compute | any | S | #853 (PR #854); #860 | reported |
| P0-54 | `apply_template` overclaims | Claims wiring it does not do. | file | 10.0 | S | #784 | reported |
| P0-55 | mm to nm truncation | `mm_to_nm` truncates (4.1 mm becomes 4 099 999 nm); the error reaches the CPL. | compute | any | S | #774 | reported |
| P0-56 | Library refresh never converges | Writes `(type default)` strokes and never reaches a no-op plan. | IPC | 10.0 | S | #970; PR #972 | reported |
| P0-57 | Placed fields lack library style | `place_component` builds fields without library layer or style. | IPC | 10.0 | S | #971 | reported |
| P0-58 | IPC footprint writes drop units | Root cause is in KiCad (kicad work item 25859). | IPC | 11 | S | #941 | reported |
| P0-59 | Edge-mounted parts fail scoring | `score_placement` hard-fails edge-mounted parts. | compute | any | S | #951 | reported |
| P0-60 | Flaky rotation readback | Stale rotation after `UpdateItems` plus save. | IPC | 10.0 | M | #221 | reported |
| P0-61 | Metadata changes misreported | `set_footprint_metadata` drops attributes silently; a failed readback is reported as "not changed". | file | 10.0 | S | #895 (PR #906); #905 | reported |
| P0-62 | `delete_symbol` misses tab-indented | Cannot find symbols in tab-indented libraries. | file | 10.0 | S | #907; PR #949 | reported |
| P0-63 | Exports fail without layers | `export_svg` / `export_pdf` fail when no layers are given. | cli | 10.0 | S | #465; PR #919 | reported |
| P0-64 | JLCPCB export rejects `-` | Designators containing `-` are rejected. | compute | any | S | #904 | reported |
| P0-65 | Schema gate wrong branch | Reports the wrong `oneOf` branch. | n/a | any | S | #932 | reported |
| P0-66 | Journal conflicts mislabelled | `create_project` mislabels journal conflicts. | n/a | any | S | #945 | reported |
| P0-67 | Output filename ignored | `export_schematic_svg` ignores the output filename. | cli | 10.0 | S | #291 | reported |
| P0-68 | Field placement edge cases | Edge cases in field placement. | file | 10.0 | S | #521 | reported |
| P0-69 | Windows long paths | `\\?\` paths mishandled. | n/a | any | S | #673, #858; PR #859 | reported |
| P0-70 | macOS ENOTCONN mapping | Wrong error mapping. | n/a | any | S | #711; PR #911 | reported |
| P0-71 | Label UUID | Label UUID handling. | file | 10.0 | S | #746; PR #801 | reported |
| P0-72 | Float noise | Float formatting noise in writes. | file | 10.0 | S | #747, #766, #828, #838; PRs #830, #807, #839, #846 | reported |
| P0-73 | Pin tools omit mirror | Pin position tools ignore symbol mirror. | compute | any | S | #855; PR #871 | reported |
| P0-74 | macOS focus | Focus handling on macOS. | n/a | any | S | #869; PR #873 | reported |
| P0-75 | Unreproduced router report | Router problem, not reproduced. | IPC | 10.0 | S | #406 | reported |
| P0-76 | Windows pipe squatter | Another process can squat the pipe name. | n/a | any | S | #531 | reported |
| P0-77 | Test fixture | Test fixture issue. | n/a | any | S | #556 | reported |
| P0-78 | Misleading lock message | "may hold this board open" message misleads. | n/a | any | S | #732 | reported |
| P0-79 | Java Dock icons | Java Dock icons appear on macOS. | n/a | any | S | #834 | reported |
| P0-80 | Misleading zone refusal | Zone refusal gives the wrong reason. | n/a | 10.0 | S | #861 | reported |
| P0-81 | PWR_FLAG numbered `#PWR` | PWR_FLAG gets a `#PWR` reference. | file | 10.0 | S | #888 | reported |
| P0-82 | Inconsistent argument names | Argument names differ between tools. | n/a | any | S | #933 | reported |
| P0-83 | Zero-length stub | A zero-length wire stub is written. | file | 10.0 | S | #950 | reported |

### P1. Missing primitives for everyday layout and schematic work

**Why next.** A model that lacks a primitive improvises: it deletes and re-creates, edits files behind the editor, or stops and asks. The external test in section 1 (finding 8) shows that this costs more than the missing tool would. Most PCB items here need only a wrapper around an API message KiCad 10 already has, and most schematic items are file edits next to existing ones. The proto resync (P1-01) comes first because several PCB items depend on fields only the newer definitions carry. Statuses of `proto-only` mean the handler was not checked; R1 sized everything on the assumption that it works.

#### P1, board (live, IPC)

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P1-01 | Proto resync to 10.0.8 | Resync all proto files; record provenance; `cargo xtask` drift compare; capability matrix. 10.0.7-rc1 adds document headers on `BeginCommit` / `EndCommit`. | IPC | 10.0.8 | M | Lead; R1 section 0 (`3b4b961`, `dd49a86`); #616; #500; `DEV.md:139` | code-read, reported |
| P1-02 | Teardrops | Pad and via teardrop settings; zone teardrop type (`build_zone` hard-codes `TDT_NONE`). Depends on P1-01. | IPC | 10.0.7 | M | Lead: `PadTeardropSettings`; `builders.rs:388`; `board_types.proto:671, 724-747`; R4 (kicad-mcp-pro) | source-verified |
| P1-03 | List vias | Position, net, type, drill, size, layer span, tenting, locked. `add_via` returns no UUID. | IPC | 10.0 | S | R1 sections 2, 4; #500; R4 | proto-only |
| P1-04 | Delete vias and other items | No tool deletes vias, zones, arc tracks, groups, barcodes or reference images. Generic type-checked delete with readback. | IPC | 10.0 | S | R1 known gaps; #500; closed PR #569; PR #921 | code-read |
| P1-05 | Via types and padstack options | Blind, buried, micro; drill span; per-layer size; tenting, covering, plugging, capping, filling; unconnected-layer removal; backdrill; post-machining. | IPC | 10.0 | S | R1: `ViaType` :457-476, `DrillProperties` :390-416; `builders.rs:254-304`; R4 | proto-only |
| P1-06 | Edit or move a via in place | Update via properties and position without delete and re-create. | IPC | 10.0 | S | R1 (`UpdateItems` Via); #500 | proto-only |
| P1-07 | Arc tracks | `query_traces` drops arcs; `delete_trace` refuses them; no `add_arc_track`. `build_track_arc` exists. | IPC | 10.0 | S | R1: `client.rs:3012`, `builders.rs:219`; R4 | code-read |
| P1-08 | Track lock and in-place edit | No locked state read or write. `modify_trace` deletes and re-creates, so the UUID changes. | IPC | 10.0 | S | R1 track row | code-read |
| P1-09 | Bulk copper transaction | Apply, delete and re-net many tracks and vias in one commit, with explicit UUID targets. | IPC | 10.0 | M | #825 (reporter has a kipy reference script) | reported |
| P1-10 | List zones | Name, net, layers, priority, fill mode, filled flag, rule-area flags. No zone inventory today. | IPC | 10.0 | S | R1; #431; PR #910 | proto-only, reported |
| P1-11 | Delete a zone or rule area | Delete by observed UUID, then refill. | IPC | 10.0 | S | #431; PR #910 | reported |
| P1-12 | Edit a zone | Priority, name, net, clearance, min width, thermal spokes, island mode, hatch, border, outline, layers, locked. Priority settable only at creation. | IPC | 10.0 | S | R1: `pcb_board.rs:528`, `builders.rs:399` | proto-only |
| P1-13 | Richer zone creation | Hatched fill, multi-layer, outline holes, island area, spoke width/gap/angle, border style, graphical zones. Fixed values today. | IPC | 10.0 | S | R1: `builders.rs:344-411`, `HatchFillSettings` | proto-only |
| P1-14 | Refill selected zones only | Pass the zone list. Contradiction to test first (section 1). | IPC | 10.0 | S | R1: `board_commands.proto:232-238`; `API_MIGRATIONS.md` | proto-only |
| P1-15 | Rule areas and keepouts | Keepout copper, vias, tracks, pads, footprints; placement areas by sheet, component class, group, design block. | IPC | 10.0 | S/M | #715; R1: `RuleAreaSettings` :759-770; R4 (jlcmcp, blwfish) | proto-only, reported |
| P1-16 | Footprint attributes | DNP, exclude from BOM and position files, not in schematic, courtyard exempt, mounting style, mask bridges. Today only set by sync. | IPC | 10.0 (mask bridges 9.0.7) | S | R1: `FootprintAttributes` :1045, `pcb_sync.rs:1267`; R4 test | proto-only |
| P1-17 | Footprint rule overrides | Clearance, mask and paste margin and ratio, zone connection. | IPC | 10.0 | S | R1: `FootprintDesignRuleOverrides` :1065 | proto-only |
| P1-18 | Footprint fields and reference rename | Rename reference; move, size, rotate, hide, relayer, knockout Reference, Value, Datasheet, Description and custom fields. #836 contradiction to test first. | IPC | 10.0 (`visible` 9.0.1) | S/M | R1: `Field` :1027-1035; #829; #456 (rename, board-owned only); #836 | proto-only, reported |
| P1-19 | Reset placed text from library | Reset Reference/Value text size and style to the library footprint. | IPC / file | 10.0 | S | #967; PR #969 (after #820) | reported |
| P1-20 | Lock and unlock items | Footprints, tracks, vias, zones, shapes. | IPC | 10.0 | S | R1 (`LockedState`); #455 | proto-only, reported |
| P1-21 | Footprint details readout | All fields, attributes, overrides, symbol path, sheet, filters, net ties, jumpers, locked; pad `symbol_pin`. | IPC | 10.0 (sheet fields 9.0.7) | S | R1 sections 2, 4 | proto-only |
| P1-22 | Back-side placement | `place_component` refuses back-side placement. | IPC | 10.0.6 | M | R3: `pcb_components.rs:130,196`; PR #921 | code-read |
| P1-23 | Footprints with custom pads | Refused on the typed placement path. PR places them as bounding rectangles; needs a scope decision. | IPC | 10.0 | M | R3: `pcb_components.rs:235`; PR #741 | code-read |
| P1-24 | Swap one placed footprint | Replace one board footprint with another library footprint, keeping position and nets. Existing tools not checked for this. | IPC | 10.0 | M | R4 (ProductOfAmerica test); R2 (`replace_component` exists on schematic side) | unverified |
| P1-25 | Edit footprint graphics on board | Lines, arcs, circles, text of a placed footprint. `edit_board_footprint_graphic` handles single-outline polygons only. | IPC | 10.0 | M | R1 footprint row | code-read |
| P1-26 | Net ties, jumpers, private layers | Read and write on placed footprints. | IPC | 10.0 | S | R1 footprint row | proto-only |
| P1-27 | Pad properties on the board | Net, clearance override, zone connection, thermal spokes, mask and paste, pad-to-die length and delay, unconnected-layer removal. | IPC | 10.0 (delay 10.0.0) | M | R1 (standalone pad update untested); #456; PR #921; R4 | proto-only, reported |
| P1-28 | Board graphics on any layer | Lines, arcs, circles, rectangles with radius, polygons with holes, bezier; stroke style, fill, net on copper. Builders exist. | IPC | 10.0 | S | R1: `builders.rs:501-628`; known gap (silkscreen lines) | proto-only |
| P1-29 | List and edit board graphics | Full geometry readback (today origin only) and in-place edits. | IPC | 10.0 | S/M | R1: `IpcGraphic`, `types.rs` | code-read |
| P1-30 | Non-rectangular outline | Polygon and circle outlines. Shape must come from DXF, SVG or a template, not model recall. | IPC / file | 10.0 | M | #471; R4 (mixelpixx) | reported, external |
| P1-31 | Footprint-owned Edge.Cuts | Recognise them in outline and placement analysis, read only. | IPC | 10.0 | S | #351 (waiting for a fixture) | reported |
| P1-32 | Board text options and editing | Mirror, justify, bold, italic, font, line spacing, keep-upright, knockout, hyperlink, locked; edit content. | IPC | 10.0 | S | R1 `BoardText` row; #457 | proto-only, reported |
| P1-33 | Board text boxes | Create and edit `BoardTextBox`. | IPC | 10.0 | S/M | R1 | proto-only |
| P1-34 | Dimensions | Aligned, orthogonal, radial, leader, center; units, precision, text, arrows. | IPC | 10.0 | M | R1 `Dimension` :954 | proto-only |
| P1-35 | Board groups | Create, rename, list members, ungroup. | IPC | 10.0 | M | R1 `Group` :1010 | proto-only |
| P1-36 | Barcodes | QR, DataMatrix, Code39, Code128, microQR on silkscreen. | IPC | 10.0.6 | M | R1 `Barcode` :535 | proto-only |
| P1-37 | Reference images | Add an image with scale, position, layer. | IPC | 10.0.6 | M | R1 `ReferenceImage` :991 | proto-only |
| P1-38 | Board title block | Write all 13 fields; read all (today 4 of 13). Version contradiction to test (section 1). | IPC | 10.0.1 or 10.0.6 | M | R1: `editor_commands.proto:408`, `IpcTitleBlock`; #714; PR #943 | proto-only, reported |
| P1-39 | Live copper layer count | Change layer count with the board open; `add_layer` needs KiCad closed. Destructive on removal. | IPC | 10.0 or 10.0.6 | M | R1: `pcb_board.rs:1148`, `board_commands.proto:72`; #721; closed PR #566 | proto-only, reported |
| P1-40 | Active layer | Get and set the active layer. Contradiction to test first (section 1). | IPC | 10.0 | M | R1: `board_commands.proto:365-375`; #610 | proto-only |
| P1-41 | Visible layers and appearance | Visibility, inactive-layer dim, net colours, flip view, ratsnest display. | IPC | 10.0 | M | R1: board:342-433 | proto-only |
| P1-42 | Board origin | Get and set grid and drill origin, used by fab outputs. | IPC | 10.0 | M | R1: board:103-110 | proto-only |
| P1-43 | Text variables | Get, set, expand `${VAR}` live; `.kicad_pro` `text_variables` when closed. CLI `-D`: P5-01. | IPC / file | 10.0 | M | R1: project:54-75; R2 section 3, candidate 25 | proto-only, code-read |
| P1-44 | Graphics defaults | Read per-layer-class default text and line widths. Writing: P7-06. | IPC | 10.0 | M | R1: board:85 | proto-only |
| P1-45 | Live netclass editing | `create_netclass` refuses while KiCad holds the board. In progress on branch `feat/netclasses-over-ipc`. | IPC | 10.0 | M | Lead: `common/api/api_handler_common.cpp`; R1; R2 K5 | source-verified |
| P1-46 | Effective netclass, all fields | `GetNetClassForNets` wrapper exists but returns 4 fields; used only by DSN export. | IPC | 10.0 | S | R1: `client.rs:1970` | code-read |
| P1-47 | Items by net or netclass | Copper items of given nets or netclasses; netclass filter on `GetNets`. | IPC | 10.0 | M | R1: board:147-182 | proto-only |
| P1-48 | Copper-connected items | Items connected to seed items (name it apart from the schematic tool). | IPC | 10.0.6 | M | R1: board:197 | proto-only |
| P1-49 | Pad outline and padstack layers | Exact pad polygon per layer; where pads and vias are flashed. | IPC | 10.0 | M | R1: board:246, 277 | proto-only |
| P1-50 | Text extents and glyph shapes | For silk-overlap checks against real glyphs. | IPC | 10.0 | M | R1: base:54, 71 | proto-only |
| P1-51 | Hit test and fetch by ID | Code lists all items then filters today. | IPC | 10.0 | M | R1: editor:225, 379 | proto-only |
| P1-52 | Save a board copy | Save to a path without switching the open document. | IPC | 10.0 | M | R1: editor:76 | proto-only |
| P1-53 | Revert board | Revert to disk. Dangerous; needs confirmation design. | IPC | 10.0 | M | R1: editor:85 | proto-only |
| P1-54 | Copy and paste as s-expression | Paste once returned OK with zero items, so it needs readback machinery. | IPC | 10.0 | L | R1: `builders.rs:244-253`, `client.rs:2863-2868` | code-read |
| P1-55 | Interactive move | Editor blocks until the user finishes. Probably not wanted. | IPC | 10.0 | L | R1: board:487 | proto-only |

#### P1, schematic (file editing)

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P1-56 | Schematic title block | Title, date, revision, company, comments 1 to 9; get and set. Node is kept unmodelled today. | file | 10.0 | S | R2 K1: `SE/mod.rs:126`, `T/sch_batch.rs:915`; #714; PR #944 (stacked on #943) | code-read, reported |
| P1-57 | Symbol attributes (DNP and others) | DNP, exclude from BOM, board, simulation, position files. Model has most fields; no tool sets them. `in_pos_files` not modelled. | file | 10.0 | S | #415 (PR #816 [D]); #435; R2 K3: `SE/symbol.rs:39-44, 618`; R4 | code-read, reported |
| P1-58 | Schematic graphics | Add, edit, delete rectangle, circle, arc, polyline, bezier with stroke and fill. | file | 10.0 | M | R2 K2, candidate 3 | code-read |
| P1-59 | List schematic items with UUIDs | Text, text boxes, graphics, images, tables, buses, bus entries, no-connects, labels. Makes P0-37 usable. | file | 10.0 | S | R2 D7, candidate 4; #800 | code-read |
| P1-60 | Schematic text editing | List, edit, move, delete free text by UUID; hyperlinks; text boxes. | file | 10.0 | S | #563; closed PR #564; R2 candidate 5 | reported, code-read |
| P1-61 | Power symbol choice and scope | `lib_id` is hard-coded `power:<net>`; add symbol choice, value as net name, `(power local)`. | file | 10.0 | S | R2 K4: `T/sch_wiring.rs:2152`; #876 | code-read, reported |
| P1-62 | Power symbol on a pin, batch | Place by reference and pin number; join nearby pins on one rail. | file | 10.0 | M | #727 (PR #782); #856 (PR #857 [D], after #782) | reported |
| P1-63 | Netclass directive labels | `netclass_flag` with `Netclass` / `Component Class` fields. Not even parsed today; analysis must learn it. | file | 10.0 | M | R2: `konnect-sexp/src/schematic.rs:200-205`, candidate 7 | code-read |
| P1-64 | Mirror and unit after placement | Mirror a placed symbol (optionally one unit); change or move a single unit. | file | 10.0 | M | #450 (PR #808); #467; R2 candidate 8 | reported, code-read |
| P1-65 | Body style and pin alternates | De Morgan `body_style`; `(pin "N" (alternate ...))`. | file | 10.0 | M | R2 symbol row, candidate 8 | code-read |
| P1-66 | Field delete and style | Delete a field; show_name, font size, bold, italic, justify, `do_not_autoplace`. | file | 10.0 | S | #925 (fork branch `davidtdab/konnect-contrib:feat/remove-component-fields`); R2 candidate 9 | reported, code-read |
| P1-67 | Batch edit with placements and unit | `batch_edit_schematic_components` should accept `field_placements` and `unit`. | file | 10.0 | S | #898 | reported |
| P1-68 | Label rename, retype, shape | Rename in place (keeps UUID and shape), retype, change shape, font. Return a connectivity diff. | file | 10.0 | S | #953; R2 candidate 10 | reported, code-read |
| P1-69 | Batch hierarchical labels | 26 labels take 26 calls today. | file | 10.0 | S | #468 | reported |
| P1-70 | Sheet styling, fields, attributes | Border and fill colour, custom fields and placement, sheet DNP, BOM, board, simulation flags. | file | 10.0 | S | R2 sheet row, candidate 11; `SE/sheet.rs:145-158` | code-read |
| P1-71 | Custom page size | `User` size with width and height; the model preserves it but the enum omits it. | file | 10.0 | S | R2: `T/sch_hierarchy.rs:56-80`, `SE/mod.rs:103-114` | code-read |
| P1-72 | Tables, images, rule areas, groups | Schematic tables, base64 images, rule areas, real `(group ...)`. Accept each through `sch export svg`. | file | 10.0 | M each | R2 candidate 13; D6 | unverified |
| P1-73 | Wire style and endpoints | Stroke width, style, colour; move a wire endpoint. | file | 10.0 | S | R2 wire row; `SE/wire.rs:10` | code-read |
| P1-74 | Bus and bus entry listing and edit | Buses are only read internally; no list or edit. | file | 10.0 | S | R2 bus rows; `T/sch_bus.rs` | code-read |
| P1-75 | Junction and no-connect details | Junction diameter and colour; list no-connects. | file | 10.0 | S | R2 junction and no-connect rows | code-read |
| P1-76 | Annotation options | Annotate whole hierarchy from root with cross-sheet duplicates; clear annotation; start number, sheet x 100. | file | 10.0 | M | #463 (fork branch `davidtdab/konnect-contrib:fix/463-annotate-hierarchy`); R2 annotation row | reported, code-read |
| P1-77 | Full part data at placement | Datasheet, Footprint, LCSC/MPN in `add_schematic_component`; batch datasheet enrichment. | file | 10.0 | M | #360 (depends on #359) | reported |
| P1-78 | Rigid region move | `move_region include:"all"` with connectivity checked before and after. Distinct from rejected #315. | file | 10.0 | M | #896; PR #934 | reported |
| P1-79 | Global label intersheet references | No intersheet-reference field handling. | file | 10.0 | S | R2 global label row | code-read |
| P1-80 | Embedded files and fonts | `embedded_files` and `embedded_fonts` are protected, not editable. | file | 10.0 | M | R2: `T/sch_batch.rs:919` | code-read |

### P2. Trust and feedback loops

**Why.** A model edits better when it can see and check what it did. DRC and ERC already give KiCad the verdict. These items extend that to the board picture, to routability, to stated design intent, and to whether an edit changed connectivity. They also make results easier to trust: bounded responses, an honest statement of what was not checked, and a history of results. Most are computation over data the server already reads, or kicad-cli runs.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P2-01 | Per-layer PCB images | Per-layer PNG with region crop, zoom and net highlight. Today: top 3D view and SVG export only. | cli, compute | 10.0 | S/M | R4 gap 8 ([kicad-mcp-pro](https://github.com/oaslananka/kicad-mcp-pro/blob/main/docs/tools-reference.generated.md)); reuse the schematic rasteriser | external |
| P2-02 | PCB visual baseline and diff | Like the existing schematic baselines. | cli, compute | 10.0 | M | R4 gaps 8, 13; [KiRI](https://github.com/leoheck/kiri) (MIT) | external |
| P2-03 | 3D render options | `pcb render`: side, quality, preset, zoom, pan, pivot, rotate, perspective, background, lighting, stackup colours, variant, JPEG. | cli | 10.0 | S | R2 candidate 32, section 4; R4 gap 8 | code-read |
| P2-04 | Ratsnest and unrouted metrics | Per-net airwire count, spanning-tree length, total unrouted length, crossings. No API query; compute from pads and copper. | compute | any | S | R4 gap 9; R1 section 5 (approximation via `GetConnectedItems`) | external, proto-only |
| P2-05 | Routability gate | Require a routability estimate (crossings, density) before routing starts, so placement gets revised. | compute | any | M | R4 section 3 ("force revision", [Anthropic](https://anthropic.com/research/claude-electronics-design)) | external |
| P2-06 | Checkable design intent | Typed assertions (capacitance between nets, cap-to-pin distance, netclass, impedance, length, current, edge placement); `validate_intent` returns pass, fail or unknown with evidence. | compute | any | S/M | R4 gap 1; today `add_design_rule` stores prose nothing checks | external, code-read |
| P2-07 | Custom DRC rule authoring | List, add, replace, delete `.kicad_dru` rules with conditions and constraints. Syntax check is unreliable (see note). | file, cli | 10.0 | M | R2 candidate 23, K6; #881; R4 gap 3 | code-read, reported |
| P2-08 | Judge rules by content | Judge a `.kicad_dru` by its rules, not its presence. Increment 1: truthful refusal; 2: an enumerated subset of `min` constraints. | file | 10.0 | M | #685 (no general interpreter) | reported |
| P2-09 | Rule conflict check | Detect conflicting or shadowed rules. | compute | any | M | R4 (kicad-mcp-pro `drc_check_rule_conflicts`) | external |
| P2-10 | ERC and DRC severities | Set rule severities; ERC pin map. 47 ERC and 62 DRC severities in a real project file. | file | 10.0 | S | R2 section 3, candidate 24 | code-read |
| P2-11 | DRC and ERC exclusions | Read and write exclusions. R2 calls it a file edit; #434 says do not reverse-engineer exclusion hashes. | file | 10.0 | S/L | R2 candidate 24; #434 | code-read, reported |
| P2-12 | Per-rule DRC counts | Counts per rule in the `run_drc` summary. | cli | 10.0 | S | #952 | reported |
| P2-13 | Shared outcome envelope | DRC source and outcome envelope; only 4 tools use the shared `outcome` envelope. | n/a | any | M | Reliability debt; #119, #574; `docs/RELIABILITY_CONTRACT.md` | reported |
| P2-14 | Live versus saved board contract | One `board_source: auto/live/saved` contract across all readers. | IPC, file | 10.0 | L | #574 (roadmap item 1) | reported |
| P2-15 | Netlist parity after edits | Compare the netlist against a saved baseline after every mutating batch. | cli, compute | 10.0 | M | R4 section 3 | external |
| P2-16 | Git checkpoints and netlist diff | Checkpoint commits; semantic netlist diff. | ext (git), compute | any | S/M | R4 gap 13; kicad-mcp-pro `vcs_*` | external |
| P2-17 | Gate history and CI review | Persist review, DRC and ERC results per commit; a CI job that posts the review on a pull request. | n/a | any | S/M | R4 gap 23 ([kicad-happy](https://github.com/aklofas/kicad-happy)) | external |
| P2-18 | Edit-scoped re-check | Re-check only what an edit could affect. | compute | any | M | R4 matrix (kicad-mcp-pro `project_revalidate_after_edit`) | external |
| P2-19 | Review markers on the board | Annotate the board with findings as DRC markers. | IPC | 10.0 | M | R1: `InjectDrcError` (board:321) | proto-only |
| P2-20 | Response size limits | `limit` and `truncated` on DRC and search tools; three tools return 43 to 166 KB single-line JSON; compact batch placement output. | n/a | any | S | #119; #659; #893 | reported |
| P2-21 | Coverage per error class | Report covered, partial or blind per class; an empty result never counts as clean. | compute | any | M | R4 section 3 ([source](https://www.electronicdesign.com/55417978)) | external |
| P2-22 | Declare stale zone fills | Routing tools leave fills stale without saying so. Maintainer advised against a `zones_stale` field. | IPC | 10.0 | S | #661 | reported |
| P2-23 | Acknowledge routed pad-net change | In sync, bound to the plan revision; never a blind force. | IPC | 10.0 | M | #451 | reported |
| P2-24 | "Decision needed" results | Structured result at ambiguity instead of a guess. | n/a | any | M | R4 section 3 ([eevblog](https://www.eevblog.com/forum/eda/claude-code-for-pcb-design/)) | external |
| P2-25 | Schematic readability lints | Wires through symbol bodies, off-grid items, field autoplace, crossing count. | compute | any | S/M | R4 gap 27 (mixelpixx) | external |
| P2-26 | Silkscreen checks | Silk-to-pad check against real glyph shapes (see P1-50). | compute | 10.0 | S | R4 matrix (kicad-mcp-pro) | external |
| P2-27 | Cost and time per session | Show token cost and time per session. | n/a | any | S | R4 section 3, complaint 7 | external |
| P2-28 | Benchmark and example boards | Publish measured results and example outputs. | n/a | any | M | R4 section 3, complaint 9 ([HN](https://news.ycombinator.com/item?id=50085553)) | external |

Note on P2-07: R2 and R4 propose checking rule syntax by running `pcb drc` on a copy. #881 reports that one malformed rule makes `kicad-cli pcb drc` silently ignore all custom rules with exit code 0. A check must look for the rules' effect, not the exit code.

### P3. Engineering analysis

**Why.** This is where AI review can find real design faults, and where other tools are ahead. All of it is either closed-form computation over data the server reads, or an external solver run as a subprocess. None of it changes the board, so the risk is wrong advice rather than a wrong board. Each result must state its method and its limits, and fabricated models are forbidden.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P3-01 | Impedance calculator | Microstrip, stripline, coplanar, differential from `get_board_stackup`; width for a target; tolerance spread. | compute | any | S | R4 gap 5 (kicad-mcp-pro, jlcmcp, [Z0 Suite](https://forum.kicad.info/t/z0-suite-0-99-3-a-test-release-of-an-impedance-and-signal-analysis-plugin-for-kicad-10/72139)) | external |
| P3-02 | Stackup write and synthesis | Write the stackup; generate one for given interfaces. Not possible over IPC on KiCad 10 (section 3); file writer rejected upstream (section 4). | file / IPC | 11 (IPC) | L | R4 gap 5; R1 section 5; #716 | proto-only, reported |
| P3-03 | Length and delay report | Net length and propagation delay per layer, including vias. | compute | 10.0 | M | R4 gap 4 | external |
| P3-04 | Matching groups and tuning rules | Group matching (byte lanes, pairs); write KiCad 10 tuning profiles and length/skew rules. | file | 10.0 | M | R4 gap 4; R2 (`tuning_profiles`, netclass `tuning_profile`) | external, code-read |
| P3-05 | Meander insertion | Insert meanders as segments, or delegate to KiCadRoutingTools (MIT). Tuning generators are not in the IPC API. | IPC, ext | 10.0 | L | R4 gap 4 (UNVERIFIED for generators); R1 section 5 | external |
| P3-06 | Current capacity and path IR drop | IPC-2152 width for current; resistance and drop along routed copper. | compute | any | S/M | R4 gap 14 | external |
| P3-07 | FEM IR drop | Wrap padne (GPL-3.0, CLI `padne solve`) as a subprocess. | ext | 10.0 | L | R4 gap 14 ([padne](https://github.com/atx/padne)) | external |
| P3-08 | SPICE loop | Validate models, add directives, run OP, AC, transient, DC sweep; parse results; phase margin. Model validation absent today. | cli, ext (ngspice) | 10.0 | M | R4 gap 2; R2 (simulation fields, ERC `simulation_model_issue`) | external |
| P3-09 | EMC and SI heuristics | Plane-gap crossings, voids, via stubs, pair symmetry, crystal guard, I/O filtering, ESD path, loop area. Report coverage. | compute | 10.0 | M | R4 gap 15 (kicad-happy, kicad-mcp-pro) | external |
| P3-10 | Thermal estimates | Junction temperature from Rth and power; thermal vias per pad. | compute | any | S | R4 gap 22 | external |
| P3-11 | 2.5D thermal simulation | kicad_thermal_sim (MIT) headless. | ext | 10.0 | L | R4 gap 22 ([source](https://github.com/poealex/kicad_thermal_sim)) | external |
| P3-12 | Creepage and HV audit | Surface paths against IEC 60664-1. Whether KiCad 10 DRC has a native creepage constraint is UNVERIFIED. | compute | 10.0 | S/M | R4 gap 25 (kicad-tools) | external, unverified |
| P3-13 | Full-wave SI | gerber2ems (Apache-2.0) plus openEMS (GPL-3.0) as subprocesses; S-parameters. | ext | 10.0 | L | R4 gap 26 | external |
| P3-14 | Enclosure fit | Import an enclosure STEP; check 3D bodies for collision and keep-in. | ext | 10.0 | L | R4 gap 28 | external |
| P3-15 | FMEA report | Failure mode report from the netlist and parts. | compute | any | S/M | R4 gap 29 (circuit-synth) | external |
| P3-16 | Pinout to firmware | Firmware header or devicetree from MCU pin nets. | compute | any | S | R4 gap 29 (Seeed) | external |

### P4. Parts and libraries

**Why.** Invented part data is a top complaint about AI PCB tools (section 6). Grounding every part fact in a catalogue or datasheet answers that. The library authoring gaps are where models already do well (datasheet to symbol and footprint) but this server cannot express what stock KiCad libraries contain. Library edits are file edits with the usual overwrite risk when the symbol or footprint editor holds the library.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P4-01 | Distributor sourcing | DigiKey, Mouser, Nexar search; price breaks, stock; per-supplier order files. User keys; allow keyless tiers. | ext | any | M | R4 gap 6 | external |
| P4-02 | Lifecycle, derating, policy | Lifecycle and EOL; derating against operating conditions; sourcing policy. | ext, compute | any | M | R4 gap 6 (kicad-mcp-pro) | external |
| P4-03 | Cost estimate | Flat $0.05 per part; ignores LCSC prices and the Extended fee; points to a nonexistent `generate_bom`. | compute | any | S | #793 | reported |
| P4-04 | JLCPCB search | Value words match as substrings; categories cannot be discovered. | compute | any | S | #786; PR #942 (part 1) | reported |
| P4-05 | Datasheet extraction | Download, check it matches the MPN, extract limits, application circuit, pinout with page references; cache per MPN. | ext, compute | any | M/L | R4 gap 7; #360 (one HTTP call per part) | external, reported |
| P4-06 | Edit pins of an existing symbol | Add, edit, remove pins; hidden pins; per-pin text size. Pins are create-only today. | file | 10.0 | M | R2 symbols table; #891; PR #948 (hide pins in `create_symbol`) | code-read, reported |
| P4-07 | Pin alternate functions | Library alternates (common in stock libraries). | file | 10.0 | M | R2 symbols table | code-read |
| P4-08 | Unit names and flags | `unit_name`; interchangeable and locked units. | file | 10.0 | S | R2 symbols table | code-read |
| P4-09 | De Morgan body styles | `(body_styles demorgan)` (32 stock symbols). | file | 10.0 | M | R2 symbols table | code-read |
| P4-10 | Derived symbols | `extends` (12,326 stock uses). | file | 10.0 | M | R2 symbols table | code-read |
| P4-11 | Power symbol definition | `(power global\|local)` in `create_symbol`. | file | 10.0 | S | R2 symbols table; #876 | code-read, reported |
| P4-12 | Symbol body graphics | Bezier, text, text box. | file | 10.0 | S | R2 symbols table | code-read |
| P4-13 | Symbol properties | Keywords, footprint filters, description, default footprint, custom fields; Value; rename or remove fields. Footprint and Description written empty. | file | 10.0 | S | R2: `T/library.rs:4487-4497`; #892 (PR #909); #926 (fork branch `davidtdab/konnect-contrib:feat/symbol-field-rename-remove`) | code-read, reported |
| P4-14 | Symbol attributes | `exclude_from_sim`, `in_bom`, `on_board`, `in_pos_files` hard-coded. | file | 10.0 | S | R2 symbols table | code-read |
| P4-15 | Jumper pin groups | `jumper_pin_groups`; duplicate-pin-number flag hard-coded. | file | 10.0 | S | R2 symbols table | code-read |
| P4-16 | Rename, copy, import symbols | Rename, copy, edit, import from another library. | file | 10.0 | S | R2 candidate 20 | code-read |
| P4-17 | Off-grid pin warning | Warn when a pin is off the 1.27 mm grid (silent non-connection otherwise). | compute | any | S | #466 | reported |
| P4-18 | Add symbol not yet embedded | `update_symbols_from_library` cannot add one. | file | 10.0 | S | #470 | reported |
| P4-19 | Symbol from pin table | Generate a symbol from a pin-table CSV. | file | 10.0 | M | R4 gap 21 | external |
| P4-20 | Pad type `connect` | Missing (175 stock footprints). | file | 10.0 | S | R2: `T/library.rs:106` | code-read |
| P4-21 | Pad shapes | Custom (1,976 stock pads), trapezoid with `rect_delta`, chamfer. | file | 10.0 | M/L | R2 footprints table; #879; #296 | code-read, reported |
| P4-22 | Add or delete one pad | `edit_footprint_pad` edits and renumbers only. | file | 10.0 | S | R2 candidate 16 | code-read |
| P4-23 | Pad drill and properties | Oval or slot drill, drill offset, layers, rotation, mask/paste, clearance, thermal, die length, unused-layer removal. | file | 10.0 | M | R2; #296, #565; PR #621; closed PR #561 | code-read, reported |
| P4-24 | Pad fabrication property | BGA, fiducial, heatsink, mechanical, testpoint, castellated (1,364 stock footprints). | file | 10.0 | S | R2; #961; PR #962 | code-read, reported |
| P4-25 | Footprint graphics selection | Bezier; select a graphic by UUID, not only by layer. | file | 10.0 | S | R2; #225; closed PR #159 | code-read, reported |
| P4-26 | Footprint fields and text | Reference/Value position and layer, user text, `fp_text_box`; `create_footprint` writes legacy `fp_text`. | file | 10.0 | S | R2 candidate 17; #878 | code-read, reported |
| P4-27 | Footprint-level rules | Clearance, mask/paste margins, `zone_connect`, net ties, private layers, jumper groups. | file | 10.0 | S | R2 candidate 18 | code-read |
| P4-28 | Keepouts in footprints | Rule areas inside library footprints (229 stock files). | file | 10.0 | M | R2 candidate 19; #877 | code-read, reported |
| P4-29 | Footprint dimensions and groups | Missing; groups only protected. | file | 10.0 | M | R2: `T/footprint_graphics.rs:874` | code-read |
| P4-30 | Delete, rename, copy footprints | Missing. | file | 10.0 | S | R2 candidate 20 | code-read |
| P4-31 | `create_footprint` options | Skip the silk outline; back-side pads get no B.CrtYd. | file | 10.0 | S | #894 | reported |
| P4-32 | Generation controls | Rotation-aware courtyard and silk, pin-1 marker, multi-unit field placement, symbol metadata. | file | 10.0 | M | #296 (tracker) | reported |
| P4-33 | IPC-7351 generation | Land pattern from package dimensions; validate a footprint against IPC-7351. | compute, file | 10.0 | M | R4 gap 21 | external |
| P4-34 | LCSC / EasyEDA import | easyeda2kicad (AGPL-3.0) as a subprocess. | ext | 10.0 | M | R4 gap 21 ([source](https://github.com/uPesy/easyeda2kicad.py)) | external |
| P4-35 | Library refresh coverage | `update_footprints_from_library` fails closed on 29% of stock footprints (`unlocked`, `pad_prop_*`, net ties, `(point)`). | IPC | 10.0 | M | #775; `pcb_footprint_update.rs` | reported |
| P4-36 | Unregister, disable, hide libraries | Also nested `Table` entries and the design-block library table. | file | 10.0 | S | R2 library tables, candidate 21; #672 | code-read, reported |
| P4-37 | Database libraries | `.kicad_dbl` (ODBC). | file | 10.0 | L | #939 | reported |
| P4-38 | Parts reuse registry | Proven blocks pairing schematic, layout fragment and BOM data. Design blocks are the native vehicle (P7-11). | file | 10.0 | L | R4 section 3 (Diode Registry, Seeed) | external |

### P5. Manufacturing and exports

**Why.** These are mostly option pass-through on kicad-cli calls the server already makes. Each is small and low risk, because exports only read the design. Together they decide whether a designer can produce fab outputs without leaving the assistant. Variants and panelization are the two larger items.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P5-01 | Text variable pass-through | `-D` on every export, ERC and DRC call. | cli | 10.0 | S | R2 candidate 25, section 4 | code-read |
| P5-02 | ERC options | `--severity-exclusions`, `--exit-code-violations`. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-03 | BOM options and presets | `--variant`, `--preset`, `--format-preset`, sort, filter, delimiters, keep tabs and line breaks; `schematic.bom_presets`. `export_bom` sits in the pcb_export toolset. | cli, file | 10.0 | S | R2 candidate 28 | code-read |
| P5-04 | Netlist variant | `sch export netlist --variant`. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-05 | Schematic PDF options | Page list, theme, drawing sheet, variant, exclude drawing sheet, font, hop-over, PDF exclusions, `-n`. Today root sheet only. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-06 | Schematic SVG options | Pages, drawing sheet, exclude drawing sheet, `-n`, variant, font, hop-over. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-07 | Other schematic and board formats | `sch export dxf`, `ps`, `hpgl`, `python-bom`; `pcb export ps`. | cli | 10.0 | S | R2 candidate 39 | code-read |
| P5-08 | DRC options | `--all-track-errors`, `--save-board`, `--severity-exclusions`, units. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-09 | Gerber options | Board plot params, common layers, no X2, no netlist, subtract mask, precision, drill origin, aperture macros, Protel ext, check zones, variant. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-10 | Drill export tool | Own tool: format, map, report, zeros, units, oval format, origin, mirror-y, tenting. Today only inside Gerber export. | cli | 10.0 | S | R2 candidate 34 | code-read |
| P5-11 | Position file options | ASCII format, SMD only, exclude through-hole, bottom negate X, drill origin, board edge, variant. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-12 | Board PDF options | Separate and multipage modes, mirror, negative, theme, drill shapes, scale, background, variant. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-13 | Board SVG options | Page size mode, fit to board, exclude drawing sheet, mirror, negative, theme, multi mode, variant. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-14 | DXF options | Units, origin, scale, outline mode. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-15 | 3D export options and 3D PDF | DNP, variant, board only, component and net filters, include layers, fuse shapes, substitution; `3dpdf`. | cli | 10.0 | S | R2 section 4; R4 gap 24 | code-read, external |
| P5-16 | IPC-2581, ODB++, GenCAD options | Version, precision, BOM columns, revision, variant; GenCAD's 5 flags. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-17 | Board statistics | `pcb export stats --format json`; useful for cost estimates. | cli | 10.0 | S | R2 candidate 31; R4 gap 24 | code-read |
| P5-18 | Import other formats | `pcb import` (PADS, Altium, Eagle, CADSTAR, Fabmaster, P-CAD, SolidWorks). | cli | 10.0 | S | R2 candidate 37; R4 | code-read |
| P5-19 | File upgrade | `sch`, `pcb`, `sym`, `fp upgrade`; rewrites the whole file, so lock check and snapshot. | cli | 10.0 | S | R2 candidate 35 | code-read |
| P5-20 | Symbol and footprint SVG | `sym export svg`, `fp export svg` for visual checks after library authoring. | cli | 10.0 | S | R2 candidate 36 | code-read |
| P5-21 | Run a jobset | `kicad-cli jobset run`; failed job exits non-zero. Authoring stays out (section 4). | cli | 10.0 | S | R2 candidate 29; R4 gap 24; #718 | code-read, reported |
| P5-22 | Version details | `version --format commit\|about`. | cli | 10.0 | S | R2 section 4 | code-read |
| P5-23 | Design variants | List, create, delete, override; `--variant` on every export; per-variant BOM diff. Rejected upstream (section 4); settle P0-33 first. | file, cli | 10.0 | L | R2 candidate 14, D2; R4 gap 12; #717 | code-read, reported |
| P5-24 | Panelization | Wrap KiKit (MIT) as a subprocess; needs KiCad's Python. | ext | 10.0 | M | R4 gap 20 ([KiKit](https://github.com/yaqwsx/KiKit)) | external |
| P5-25 | Interactive HTML BOM | InteractiveHtmlBom (MIT); assembly docs. | ext | 10.0 | S | R4 gap 24 ([source](https://github.com/openscopeproject/InteractiveHtmlBom)) | external |

IPC-D-356 export is P0-23. Board 3D render is P2-03.

### P6. Routing and placement capability

**Why.** These raise what the assistant can produce, not whether it is correct. They depend on P0 and P1: a multilayer router or replicate tool built on missing via, zone and lock primitives would repeat the problems in section 6. Several reuse external routers, so licence and process isolation matter.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P6-01 | Multilayer Freerouting | DSN export, manifest and SES import are limited to two copper layers. Native DSN relies on the SWIG bridge, gone in KiCad 11. | IPC, ext | 10.0 | L | #449 (no Rust multilayer exporter); P8-03 | reported |
| P6-02 | Roundrect pads in DSN | 51% of stock footprints are refused. | file | 10.0 | S | #790; PR #843 | reported |
| P6-03 | Native DSN path | `native_bridge_mode=require` can never reach KiCad's exporter; native adoption refuses stock footprints. | ext | 10.0 | M | #670; #841 | reported |
| P6-04 | Rule areas in the Freerouting round trip | Two-layer rule areas; must fail closed on copper pours. | IPC, file | 10.0 | M | #722 (after P1-15) | reported |
| P6-05 | Second router | Wrap KiCadRoutingTools (MIT) on a board copy; import through plan and apply. Speed claim is one anecdote. | ext | 10.0 | M | R4 gap 16 ([source](https://github.com/drandyhaas/KiCadRoutingTools)) | external |
| P6-06 | Via stitching and fences | Compute a grid inside zones; place vias over IPC in one commit with clearance checks. Native only after KiCad 10. | IPC, compute | 10.0 | M | R4 gap 11; RF-tools (GPL-3.0) reference only | external |
| P6-07 | Replicate layout, multichannel | Match footprints by component class or sheet path. Do not vendor ReplicateLayout (GPL-2.0). | IPC, compute | 10.0 | M/L | R4 gap 17 | external |
| P6-08 | Pin and gate swap | Schematic edit plus sync; constraints from pin types. | file | 10.0 | M | R4 gap 18 | external |
| P6-09 | Test points and coverage | Coverage computed; placement over IPC. Today only `audit_power_rails` flags rails without test points. | compute, IPC | 10.0 | S/M | R4 gap 19 | external |
| P6-10 | Placement critique | Grade placement against declared floorplan intent. | compute | any | M | R4 matrix (kicad-mcp-pro, KiCadRoutingTools) | external |
| P6-11 | Locked parts are kept | User-approved positions survive any re-optimisation in placement tools. | compute | any | S | R4 section 3 (atoplace) | external |
| P6-12 | Candidate ranking | Route N variants on board copies, score, show side by side. | ext, compute | 10.0 | L | R4 section 3 (Quilter claim, vendor-reported) | external |
| P6-13 | Decoupling on MCU rails | `score_placement` 8-reference fanout limit leaves every cap unproven. Only explicit caller-supplied cap-to-pin association. | compute | any | S | #792 | reported |
| P6-14 | Silkscreen auto-placement | Place reference text automatically. | IPC, compute | 10.0 | M | R4 matrix (jlcmcp) | external |
| P6-15 | Code to schematic | Import SKiDL, atopile or Zener netlists into placed schematics. | file | 10.0 | L | R4 gap 30 | external |

### P7. Project files and configuration

**Why.** Project files carry the rules every board tool depends on: netclasses, design-rule minima, defaults, severities. This server covers a small part of them today. All are `.kicad_pro` edits, which carry the highest overwrite risk: eeschema has no IPC, so the lock file is the only signal (P0-17 comes first).

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P7-01 | Netclass fields, delete, rename | 13 of 17 class keys not settable (diff pair, microvia, wire and bus width, colours, line style, priority, tuning profile); no delete or rename. | file | 10.0 | M | R2 section 3, candidate 22; #882; #777 (PR #866) | code-read, reported |
| P7-02 | Netclass patterns and assignments | Remove patterns; wildcard or regex; explicit `netclass_assignments`; net colours. Not in the IPC API. | file | 10.0 | S | R2 section 3; R1 section 5 | code-read |
| P7-03 | Design rule minima | 14 of 19 `design_settings.rules` keys missing (annular width, hole clearance, edge clearance, silk, text, microvia, groove, mask, max error, spokes). | file | 10.0 | S | R2: `T/verification.rs:446-457`; #776, #880, #960; PRs #965, #966 | code-read, reported |
| P7-04 | Diff pair predefined sizes | `diff_pair_dimensions`. | file | 10.0 | S | R2 section 3 | code-read |
| P7-05 | Teardrop and tuning settings | `teardrop_*`, `tuning_pattern_settings`, `tuning_profiles` in the project file. | file | 10.0 | M | R2 section 3, candidate 26 | code-read |
| P7-06 | Board defaults | Text and line sizes per layer class, dimensions, pads, zones. | file | 10.0 | M | R2 section 3, candidate 26 | code-read |
| P7-07 | Schematic settings | Drawing defaults, annotation settings, `top_level_sheets`, bus aliases (authoring; analysis is P0-38). | file | 10.0 | M | R2 section 3 | code-read |
| P7-08 | Component classes | `component_class_settings`. | file | 10.0 | M | R2 section 3 | code-read |
| P7-09 | Drawing sheet | Assign a `.kicad_wks` (`page_layout_descr_file`); later author one (separate grammar). | file | 10.0 | S/L | R2 candidate 27 | code-read |
| P7-10 | Local settings file | `.kicad_prl` is only cleaned up by `rename_project`. | file | 10.0 | S | R2 section 3 | code-read |
| P7-11 | Design blocks | Save and place `.kicad_blocks`; design-block table. Format not checked on disk. `templates` is this server's own JSON. | file | 10.0 | L | R2 candidate 30; R4 gap 17 | unverified |

Severities and exclusions are P2-10 and P2-11. `.kicad_dru` is P2-07. Text variables are P1-43. Title blocks are P1-38 and P1-56. BOM presets are P5-03.

### P8. Platform, install, clients, KiCad 11 readiness

**Why.** Upstream tested the real-KiCad path only on Windows, and the plugin depends on SWIG, which KiCad 11 removes. A fork that runs on Linux and plans for KiCad 11 needs these before it claims either. Client discovery items decide whether models can find the tools at all.

| ID | Item | What is missing | How | Min KiCad | Size | Evidence | Status |
|---|---|---|---|---|---|---|---|
| P8-01 | Linux real-KiCad testing | E2E runs on Windows only (`choco install kicad --version=10.0.5`); no Xvfb live job. | n/a | 10.0 | M | R3 section 6: `e2e-kicad.yml`, `ci.yml`; #221; #256 | reported |
| P8-02 | Linux validation debt | Lock lifecycle on Linux and macOS blocked, not passed; live re-validation of the Linux wedge fix. | n/a | 10.0 | M | #769; #498 (PR #505) | reported |
| P8-03 | SWIG plugin replacement | `plugin/__init__.py` imports `pcbnew`; settings dialog, Start/Stop, process ownership need the IPC plugin. Native DSN bridge also SWIG. | n/a | 11 | L | #257; `plugin/native_bridge.py`; `TROUBLESHOOTING.md` | reported |
| P8-04 | Deprecated API fields | `TextAttributes.visible`, `NetCode` / `Net.code`, `RunAction` strings. | IPC | 11 | M | #257; `builders.rs`, `client.rs` | reported |
| P8-05 | KiCad 11 file format | Footprints as `(transform ...)` (see P0-27); paper size, schematic identity, variant and job APIs on master. | file, IPC | 11 | L | #680, #696, #771, #717, #718 | reported |
| P8-06 | KiCad nightly paths | 10.99 config dir and install root hard-coded to 10.0/9.0/8.0. | n/a | 10.99 | S | #938 | reported |
| P8-07 | Release checklist per KiCad major | Diff official file writers; open/save/DRC/ERC round trips. | n/a | any | M | #257 | reported |
| P8-08 | Tool catalogue discovery | `find_tools(query)`; clients that never refresh `tools/list`; compact tool surface and directory resource (VS Code 128-tool cap); unknown tool returns `unknown_tool`. | n/a | any | M | #598; #576, #517, #536, #420; #325; `TROUBLESHOOTING.md` | reported |
| P8-09 | Startup health | Report kicad-cli and IPC prerequisites once; re-probe IPC after startup; open a given board and wait. | n/a, IPC | 10.0 | M | #589; #425; #256 | reported |
| P8-10 | Remove deprecated cdylib | Remove `konnect_plugin` entrypoint; absorbs "explicit config falls back to defaults". Eligible after 2026-10-31. | n/a | any | M | #731; #545 | reported |
| P8-11 | Packaging | macOS x86_64 signing; Homebrew tap; KiCad PCM; quickstart docs; skills dir option; Muse docs. | n/a | any | M | #131, #154, #438, #611, #918, #922 (PR #923) | reported |
| P8-12 | Dependency bump | `sha2` 0.11; lock-file name must stay byte-identical. | n/a | any | S | #181 | reported |
| P8-13 | Relative paths | Resolve against server CWD; `file_not_found` hides the absolute path. | n/a | any | S | #537 | reported |
| P8-14 | Headless and plugin launch | `launch_kicad_ui` says launched on headless Linux; blank IPC socket breaks Codex; autostart on plugin load. | n/a | 10.0 | S/M | #764 (PR #827); #870 (PR #872); #655 | reported |
| P8-15 | Linux crash and false health | `delete_component` crash resurfaced (flatpak, Debian 13); `check_kicad_ui` reported healthy while KiCad was dead; flatpak socket paths. | IPC | 10.0 | M | #113 (closed not planned) | reported |
| P8-16 | File permissions | Enforce `0600` after creating the temp file; test under explicit umasks. | n/a | any | S | closed PR #555 | reported |
| P8-17 | Viewer studio | Vello renderer and staged editor never landed. | n/a | any | L | closed PR #85; perara/Konnect#3, #4 | reported |
| P8-18 | Release accounting | Release accounting from the roadmap. | n/a | any | S | #760 | reported |
| P8-19 | Rename | No release before the rename away from "konnect"; the old name also clashes with KDE Connect. | n/a | any | M | `CLAUDE.md`; R4 complaint 9 | code-read, external |

## 3. Cannot be done on KiCad 10 (do not promise)

These need an API that KiCad 10 lacks. Where a different path exists, it is named. R1 section 5 and the IPC-missing items from R3 are merged here.

| Capability | Why not | Instead, on KiCad 10 | Evidence | Status |
|---|---|---|---|---|
| Stackup edits (thickness, materials, permittivity, finish, impedance flag) | `UpdateBoardStackup` is declared but "not yet implemented". | Board Setup by hand. File writer rejected upstream (section 4). | `board_commands.proto:46`; kicad#25384; #716 | proto-only, reported |
| Layer type (signal, power, mixed, jumper) and layer rename | No message; `user_name` is read-only through the stackup. | Board Setup by hand. | R1 section 5 | proto-only |
| Board design rules over IPC | `BoardDesignRules` is an empty message. No minima, sizes, `.kicad_dru`, severities or exclusions. | File edits (P7-03, P2-07, P2-10). | `board.proto:190-192` | proto-only |
| Running DRC or reading markers over IPC | No run-DRC message; `KOT_PCB_MARKER` has no item message. `InjectDrcError` only adds. | kicad-cli (existing). | R1 sections 1, 5 | proto-only |
| DRC exclusions over IPC | No API; do not reverse-engineer exclusion hashes. | See P2-11. | #434 | reported |
| Netclass assignment patterns; net rename or delete | Not in the proto. KiCad 10 has no net table; a net exists by naming it on a pad. | File edits (P7-02). `add_net` refuses KiCad 10 boards. | R1 section 5; `pcb_routing.rs:354` | proto-only, code-read |
| Tables and tuning patterns (length-tuning generators) | No item messages; `GetItems` answers `AS_BAD_REQUEST`. | Meanders as plain segments (P3-05). | `enums.proto:42-47`; `client.rs:4320-4327` | proto-only, code-read |
| Interactive router (push and shove, tuning, diff-pair router) | No message, except unstable `RunAction` strings. | Freerouting (P6-01) or an external router (P6-05). | `editor_commands.proto:91-95` | proto-only |
| Undo and redo, zoom, centre, fit, switching the active document | Same as above. | None. | R1 section 5 | proto-only |
| Activating or revealing an editor; pin, pad and net cross-probe expansion | No message. | None. | `KICAD_INTEGRATION.md`; `editor_navigation.rs`; `client.rs:5209-5224` | code-read |
| Placing a footprint by library ID | No "place from library" command. | The client builds the whole `FootprintInstance` (existing). | `client.rs:3970` | proto-only |
| Plotting and exports over IPC; headless operation | Arrive in KiCad 11 per KiCad's developer docs. | kicad-cli (existing). | R4 ([dev docs](https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/for-addon-developers/)) | external |
| Schematic editing over IPC | KiCad 10.0 eeschema serves no usable item types, no revert, no `RunAction`. Master has a full API. | File editing (existing). KiCad 11. | Lead: `eeschema/api/api_handler_sch.cpp` (10.0); R1 section 3 | source-verified |
| Schematic editor observation (identity, selection, cross-probe) | Unusable on 10.x (empty project identity); works on 10.99. Synthesising identity from files was rejected. | PR #837 [R] reports it as unsupported. | #771 | reported |
| Opening, closing or switching projects; Save-As that opens the copy | Commented out in the proto. | `kicad -f` launch (P8-09). | `editor_commands.proto:55-63` | proto-only |
| Ratsnest or unrouted-connection query | Only approximated through `GetConnectedItems`. | Compute (P2-04). | R1 section 5 | proto-only |
| Per-net zone refill | KiCad refills every zone; input removed. Per-zone refill is a separate open question (P1-14). | Refill all. | `API_MIGRATIONS.md` | reported |
| Active layer | Upstream: editor-session state, no IPC. Contradicted by the proto (section 1). | Test first (P1-40). | #610; `board_commands.proto:365-375` | reported, proto-only |
| Paper size from the live board | `GetPageSettings` exists on master only. | Read the saved file. | #696 | reported |
| Pad fabrication property over IPC | Pad proto field 12 arrives in KiCad 11. | File path in the library (P4-24). | #959 | reported |
| Wire-carrying schematic move | No schematic IPC; doing it means re-implementing eeschema drag rules. | Move, reconnect, ERC. Rejected (section 4). | #315; `sch_components.rs:3235` | reported |
| Variant and job-export APIs | Schematic variant API and `RunBoardJobExport*` / `RunSchematicJobExport*` exist on master only. | File plus CLI `--variant` (P5-23); `jobset run` (P5-21). | #717, #718 | reported |
| IPC footprint writes keeping `(units ...)` | Root cause in KiCad. | None until fixed upstream. | #941; kicad work item 25859 | reported |
| Native via stitching | Arrives in KiCad after 10. | Computed stitching over IPC (P6-06). | R4 ([forum](https://forum.kicad.info/t/post-v10-new-features-and-development-news/66611/30)) | external |

**Moved out of this list.** R1 listed teardrop parameters on pads and vias as impossible. That holds only for the vendored 10.0.6 protos. They exist from 10.0.7 (P1-01, P1-02).

## 4. Decided against upstream

Upstream's rejection framework is in `GOVERNANCE.md` (30-day evidence window) and #590 (decision ladder). These decisions bound the upstream project, not this fork. The "revisit?" column is for the owner.

| Issue | Rejected item | Stated reason | Supported path upstream | Backlog items affected | Revisit? |
|---|---|---|---|---|---|
| #315 | Wire-carrying `move_connected` | No schematic IPC; would re-implement eeschema drag, stretch, junction and label rules. Reopen only if KiCad exposes it. | Move, reconnect, ERC. Rigid block move (#896) argued as different. | P1-78 | |
| #472 | File fallback for routing, diff pairs, refill, sync | kicad-cli cannot route, refill or update; the server would own routing semantics. | Live routing IPC only; autorouting via Freerouting. | none | |
| #508 | Prune unused `lib_symbols` | Not corruption; only file size. At most a separate cleanup utility with backups. | None. | none | |
| #717 | Design variants | No IPC or CLI creates variants; would own KiCad's variant models, including a version-dependent `in_bom` meaning. Asks filed: kicad#25665, #25666. | KiCad editor plus variant-aware CLI exports. | P5-23, P0-33 | |
| #718 | `.kicad_jobset` authoring | A separate product. Asks filed: kicad#25663, #25664. | Jobs editor plus `kicad-cli jobset run`. | P5-21 (run only) | |
| #716 | Closed-file stackup writer | Duplicates KiCad's writer; risk of silent mismatch. | Wait for `UpdateBoardStackup`. | P3-02 | |
| #436 | Generic raw-pcbnew or arbitrary batch bridge | Bypasses typing, document binding, undo boundaries and readback. | Typed tools. | P1-09 (typed bulk commit is not this) | |
| #771 | KiCad 10 schematic identity synthesised from files | Rejected. | Report unsupported (PR #837). | section 3 | |
| #113 | `delete_component` crashing KiCad | Closed not planned as stale; it resurfaced on Linux. | None. | P8-15 | |
| #216 | First-class Codex support | Withdrawn by its reporter to the external `neusse/konnect-codex` plugin. | External plugin. | none | |
| #545 | Explicit config falls back to defaults | Folded into the cdylib removal. | #731. | P8-10 | |
| #604 | Native `FlipItems` (reversed wontfix) | Wrongly declared missing because the vendored proto was stale. Lesson: check current KiCad first. | Shipped. | P1-01 | |
| #754, #602 | Whole-board auto-placement apply; force-directed refinement | Deliberate retirement of autonomous whole-board mutation. | Plan-only placement tools. | P6-10, P6-12 | |
| #661 | `zones_stale` response field | Maintainer advised against the field; declare the consequence instead. | Declared consequence. | P2-22 | |
| #351 | Auto-promotion of footprint Edge.Cuts | Not authorised. | Read-only recognition. | P1-31 | |
| #451, #456 | Force-sync | Never a blind force. | Acknowledged change bound to plan revision. | P2-23, P1-18, P1-27 | |
| #449 | Rust multilayer DSN exporter | Use KiCad's native exporter instead. | Native DSN plus N-layer manifest. | P6-01 | |
| #471 | Standard outlines recalled by the model | Outline must come from DXF, SVG or a template. | Imported outlines. | P1-30 | |
| #685 | General `.kicad_dru` interpreter | Only an enumerated subset of `min` constraints. | Truthful refusal, then subset. | P2-08, P2-09 | |
| #792 | Proximity-based decoupling inference | Only explicit caller-supplied associations. | Explicit association. | P6-13 | |
| #826 | Independent DRC engine | Not built. | KiCad DRC. | P0-20 | |
| #844 | Layer inference | Unknown layers are refused, not guessed. | Refusal. | P0-13 | |
| #845 | Autosave watcher | Not built. | Revision-checked writes. | P0-15 | |

## 5. Unmerged upstream work worth porting

Upstream is archived (#947: development moves to "Konnect-EDA" of Orchis.ai LLC after a contributor sign-off). Of 61 open PRs, the ones below carry real work; the rest are covered by the defect rows they fix. Before porting any, check the licensing basis once (section 1, finding 7) and keep authorship in the commits. `[R]` ready-to-merge label, `[D]` draft.

**Merge order:** #901 before #899 and #900 fixes; #782 before #781 and #857; #917 before #913, #914, #915, #916, #920; #850 before #849; #820 before #824 and #969; #965 before #966; #906 before #902 and #905; #909 before #907, #908, #926; #943 before #944; #566 is the base for #721.

| PR | What it carries | Item IDs | Depends on |
|---|---|---|---|
| #901 | Wire routing refuses to join other nets | P0-01, P0-02, P0-03 | none |
| #782 | Power symbol placed on a pin (+8429 lines) | P1-62, P0-04 | none |
| #857 [D] | Batch power symbol | P1-62 | #782 |
| #808 | Mirror with a unit selector | P1-64 | none |
| #816 [D] | Native DNP | P1-57 | none |
| #909 | Symbol Footprint, Description, fields; `set_symbol_properties` | P4-13, P0-25 | none |
| #910 | `list_zones` / `delete_zone` | P1-10, P1-11 | none |
| #921 | Schematic-less boards: pad nets, vias, fields, back-side placement (+2702) | P1-03, P1-04, P1-18, P1-22, P1-27 | proto resync advisable (P1-01) |
| #934 | Rigid region move | P1-78 | none |
| #943 | Board title block | P1-38 | none |
| #944 | Schematic title block | P1-56 | #943 |
| #948 | Hide individual pins | P4-06 | none |
| #962 | Pad fabrication property | P4-24 | none |
| #965, #966 | Design-rule minima | P7-03, P0-22 | #965 before #966 |
| #969 | Reset text from the library | P1-19 | #820 |
| #866 | Diff-pair netclass settings | P7-01 | none |
| #621 | Oval drills (waiting on author) | P4-23 | none |
| #741 | Custom pads as bounding rectangle (scope decision needed) | P1-23 | none |
| #937 | Junction rule | P0-52 | none |
| #814 | Label moves | P0-05 | none |
| #917 | Sheet instance keying | P0-07 | none |
| #931 | Private fields | P0-06 | none |
| #862 | Zone net rule | P0-18 | none |
| #864 | Inner-layer flip | P0-11 | none |
| #865 | Refuse sync updates that drop footprint children | P0-19 | none |
| #820 | Synced footprint fields | P0-19 | none |
| #824 [D] | Symbol fields on synced footprints | P0-19 | #820 |
| #956 | Exclude-from-BOM parts in sync | P0-19 | none |
| #852 | Revision-checked board fallbacks | P0-15 | align with `stale_read` design |
| #868 | Revision-checked `rename_project` | P0-16 | align with `stale_read` design |
| #850 | Unknown layer refused | P0-13, P0-14 | none |
| #843 | Roundrect pads in DSN (waiting on a dependency) | P6-02 | not stated |
| #847 | DSN rotation | P0-09 | none |
| #848 | DSN per-layer padstacks refused | P0-10 | none |
| #809 [R] | `copy_routing_pattern` | P0-08 | none |
| #837 [R] | KiCad 10 schematic editor reported as unsupported | section 3, P8-05 | none |
| #817 | Project-file guard | P0-17 | none |
| #906 | Metadata change report | P0-61, P0-26 | none |
| #949 | `delete_symbol` with tabs | P0-62 | #909 |
| #854 | Net pins | P0-53 | none |
| #815 | Overlap check at pin tips | P0-51 | none |
| #972 | Library refresh strokes | P0-56 | none |
| #693, #694 | Diff churn | P0-47 | none |
| #801 | Label UUID | P0-71 | none |
| #807, #830, #839, #846 | Float noise | P0-72 | none |
| #859 | Windows paths | P0-69 | none |
| #871 | Mirror in pin tools | P0-73 | none |
| #872 | Blank IPC socket | P8-14 | none |
| #873 | macOS focus | P0-74 | none |
| #911 | ENOTCONN mapping | P0-70 | none |
| #919 | Export defaults without layers | P0-63 | none |
| #942 | JLCPCB value matching | P4-04 | none |
| #955 | `batch_delete` description | P0-37 | none |
| #827 | Headless launch | P8-14 | none |
| #867 | Templates | P0-32 | none |
| #923 | Muse docs | P8-11 | none |

**Closed upstream but worth reviving, and fork branches:**

| Source | What it carries | Item IDs |
|---|---|---|
| closed PR #566 | Live layer expansion (+237/-12) | P1-39 |
| closed PR #569 | `delete_via` | P1-04 |
| closed PR #564 | Schematic text tools | P1-60 |
| closed PR #561 | Oval drills plus pad readback | P4-23 |
| closed PR #725 | Root-prefixed instance identity (rebuild needed) | P0-31 |
| closed PR #555 | Umask on created files (closed with the correct fix recorded) | P8-16 |
| closed PR #159 | Per-graphic selection prior art | P4-25 |
| closed PR #85; perara/Konnect#3, #4 | Viewer studio and maintenance tools | P8-17 |
| `davidtdab/konnect-contrib:fix/463-annotate-hierarchy` | Hierarchy annotation | P1-76 |
| `davidtdab/konnect-contrib:feat/remove-component-fields` | Remove a component field | P1-66 |
| `davidtdab/konnect-contrib:feat/symbol-field-rename-remove` | Symbol field rename and remove | P4-13 |
| dylbertram/Konnect#1 | Fork draft for #913 | P0-07 |

## 6. Recurring complaints about AI PCB tools, and the principles that answer them

From R4 sections 3 and 4. Sources are web pages read through a summarising tool; vendor numbers are vendor-reported, and single reports are single reports.

### Complaints

| # | Complaint | Source | Answered by |
|---|---|---|---|
| 1 | An MCP server did worse than plain Claude Code plus kicad-cli (3 of 18 against 18 of 18; about 1.8x context). Causes: missing primitives, direct edits forbidden, sync rewrote most footprints. | [ProductOfAmerica](https://github.com/ProductOfAmerica/mcp-server-kicad) (one test by the author of that server) | P1 tier; P0-19; principle "cover the primitives" |
| 2 | Ugly copper: traces of 10 to 30 tiny segments through SMD pins; autorouter output needing more cleanup than routing by hand. | [HN](https://news.ycombinator.com/item?id=50085553); [KiCad forum](https://forum.kicad.info/t/i-built-an-open-source-mcp-server-for-ai-assisted-kicad-workflows/68678) | P6-05, P6-10; plan before layout |
| 3 | Unasked changes: swapped parts, I2C changed to SPI, swapped RX/TX, backwards USB connector, support parts far from their ICs. | [HN](https://news.ycombinator.com/item?id=49695689); [Anthropic](https://anthropic.com/research/claude-electronics-design) | P2-15, P2-06, P6-11 |
| 4 | Invented data: nonexistent MPNs, wrong specs, cost about 2x off, missing BOM lines. | [pcbeditor.com](https://pcbeditor.com/ai-pcb-design/claude-for-pcb-design); [eevblog](https://www.eevblog.com/forum/eda/claude-code-for-pcb-design/25/) | P4-01, P4-05, P4-03, P0-28 |
| 5 | Plausible but unfounded review findings; answers that echo the prompt's framing. | [Diode](https://blog.diode.computer/llms-full.txt); [eevblog](https://www.eevblog.com/forum/eda/claude-code-for-pcb-design/) | P2-21; cite or abstain |
| 6 | Unreadable schematics: floating wires, crossings, missing symbols, a sheet copied three times. | [HN](https://news.ycombinator.com/item?id=50085553) | P2-25, P0-51 |
| 7 | Token cost with little output; sessions costing hundreds of dollars. | [HN](https://news.ycombinator.com/item?id=48368721); [eevblog](https://www.eevblog.com/forum/eda/claude-code-for-pcb-design/25/) | P2-20, P2-27, P8-08 |
| 8 | Trust and accountability: "the AI did it" is no answer to a boss. | [Siemens blog](https://blogs.sw.siemens.com/electronic-systems-design/2026/02/18/ai-in-the-engineers-seat-speeding-up-pcb-design-without-giving-up-control) | P2-17, P2-16; visible provenance |
| 9 | MCP fatigue; demand for real benchmarks. On upstream Konnect: AI-styled text, no example PDF, name clash with KDE Connect. | [KiCad forum](https://forum.kicad.info/t/kipilot-mcp-server-has-been-released-for-kicad-10-x-pcb-workflows/69049); [HN](https://news.ycombinator.com/item?id=50085553) | P2-28, P8-19 |
| 10 | High-speed, analog and RF: the verification loop is too long for AI to iterate on. | [HN](https://news.ycombinator.com/item?id=49695689) | P3 tier; say what is blind |

### Design principles

| Principle | What it means here | Source | Items |
|---|---|---|---|
| Cover the primitives | A missing primitive makes the model improvise or stall. Keep sync minimal. | Complaint 1 | P1 tier, P0-19 |
| Do not forbid direct edits so strictly the model stalls | Refuse with a structured reason and a next step, not a blanket ban. | Complaint 1 | P2-24 |
| KiCad gives the verdict, not the model | Extend from DRC and ERC to intent assertions and simulation. | [KiCad Agentic MCP](https://forum.kicad.info/t/kicad-agentic-mcp-an-ai-assistant-that-edits-your-board-through-kicads-own-api-and-lets-kicad-decide-whether-the-result-is-good/71403) (AGPL fork of Konnect v0.2.2) | P2-06, P3-08 |
| Cite or abstain; report coverage | Every finding cites a net, pin, footprint or datasheet page. Per class: covered, partial, blind. Empty is never clean. | [Electronic Design](https://www.electronicdesign.com/55417978) | P2-21, P4-05 |
| Read-only reviewer separate from the editor | Findings ranked by severity; the reviewer never edits. | [Diode](https://blog.diode.computer/llms-full.txt) | existing review tools |
| Plan before layout | Floorplan and routing plan first; critical nets first; grade against the plan. | KiCadRoutingTools; [Anthropic](https://anthropic.com/research/claude-electronics-design) | P6-10 |
| Force revision | Earlier models left boards "about 95% complete but unusable". Require a routability estimate before routing. | [Anthropic](https://anthropic.com/research/claude-electronics-design) | P2-05, P2-04 |
| Candidates instead of a single answer | Route variants on copies, score, compare. | Quilter (vendor claim); [HN](https://news.ycombinator.com/item?id=50085553) | P6-12 |
| Locked parts are sacred | Approved positions survive any re-optimisation. | atoplace ("lock > physics") | P6-11, P1-20 |
| Netlist parity after every edit | A cosmetic edit cannot silently change connectivity. | R4 section 3 | P2-15 |
| Ask, do not guess | Structured "decision needed" results at ambiguity. | [eevblog](https://www.eevblog.com/forum/eda/claude-code-for-pcb-design/) | P2-24 |
| Publish a benchmark and examples | Measured results and example boards. Public references: PCB Gym, eebench. | [Anthropic](https://anthropic.com/research/claude-electronics-design); [HN](https://news.ycombinator.com/item?id=49569366) | P2-28 |
| Reuse proven blocks | Schematic, tested layout fragment and BOM data together. | Diode Registry | P4-38, P7-11 |
| Show cost and time | Per session. | Complaint 7 | P2-27 |
| Ground every part fact | Database or datasheet lookup, never recall. | Complaint 4 | P4-01, P4-05 |

## 7. What was not checked

| Area | Caveat | Report |
|---|---|---|
| KiCad handlers | Not checked which of the 36 unwrapped commands KiCad 10.0.x implements. Exceptions: the lead's checks in section 1. | R1 |
| Accepted item types | Not checked which types `CreateItems` / `UpdateItems` accept: rule areas, groups, dimensions, barcodes, standalone pads. Every `proto-only` size assumes the handler works. | R1 |
| Live runs | Nothing was run against a live KiCad. R2 ran only `kicad-cli --help`; no export, ERC or DRC. D1, D3 and field order in symbol attributes are code reading. | R1, R2 |
| Introduction versions | The tag of the initial proto snapshot is not recorded, so "untagged" messages have no exact minimum version. | R1 |
| Code coverage | Handlers not read in full: non-PCB, library, sync, footprint update, placement, footprint graphics (grep only); `API_MIGRATIONS.md` skimmed. Tool-to-message mapping by grepping call sites. | R1, R2 |
| Out of R2's scope | PCB editing, the IPC surface, `templates`, `design_review`, `integration` toolsets; hierarchy tools with KiCad 10 multiple top-level sheets. | R2 |
| File grammar | No on-disk examples of KiCad 10 schematic tables, images, rule areas, netclass flags, groups, text boxes, `.kicad_jobset`, design-block libraries, or a KiCad-saved variant clause. Grammar from binary format strings and general knowledge. | R2 |
| Token counts | Stock library counts are raw grep counts and can overcount (for example `alternate`). | R2 |
| Upstream text | Issue comments cut at 1500 characters (#804, #826, #829, claim on #913). Most open PR bodies read to about 450 characters. | R3 |
| Upstream coverage | Discussions #165 (improvement backlog) and #224 (KiCad 10 deprecation audit) not read. Closed completed issues screened only for Linux. | R3 |
| Schematic selection on KiCad 10 | R1 reports the client decodes schematic selections; the lead's handler list for 10.0 eeschema names no selection handler; #771 says observation is unusable on 10.x. Not reconciled. | R1, Lead, R3 |
| Web sources | Pages read through a summarising fetch tool, so "read" does not mean every word. Search-summary-only claims are weaker. No Reddit threads found. | R4 |
| Pages not reached | mcp-kicad (PyPI), socket.dev mcp-server-kicad, bunnyf/pcb-mcp README, Adafruit's Flux review, "Ohmbo"; kicad-mcp-pro's 388-tool reference read in slices, partly inferred from names. | R4 |
| Products seen only via summaries | CELUS, Cadence Allegro X AI, Altium AI, DipTrace MCP, fusion-electronics-mcp, Backplane. | R4 |
| IPC in KiCad 10 for R4 ideas | R4 left open whether tuning patterns, teardrops and rule areas can be created over IPC. Now: teardrops from 10.0.7 (lead), rule areas `proto-only`, tuning patterns not in the proto. | R4, Lead |
| Licences | KiBot outputs not listed; KiCadRoutingTools MIT from README text; Z0 Suite and OrthoRoute licences not stated. Licence notes are not legal advice. | R4 |
| Vendor numbers | Quilter, DeepPCB, Flux, Cadence, Diode, Anthropic (including PCB Gym 69%) not independently verified. | R4 |
| P1-24 swap | Whether an existing tool already swaps one placed footprint was not checked. | Lead |
| Board outline shapes | R4 says outlines are rectangles with rounded corners only; R1 says `add_board_outline` writes segments and arcs. Not reconciled (P1-30). | R1, R4 |
| `SetBoardEnabledLayers` version | R1: in the first snapshot (untagged); R3 (#721): 10.0.6. Not reconciled (P1-39). | R1, R3 |

## Appendix A. The 61 API command messages (vendored protos)

From R1 section 1. "Wrapped" means `client.rs` sends the message (line given). Min version: "untagged" means present in the first snapshot, exact version unknown; "6-snap" means first seen in the 10.0.6 snapshot. Totals: 61 messages, 25 wrapped, 36 not. `schematic_commands.proto` declares none. Status for the whole table: `proto-only` for what the message does, `code-read` for the wrapper and tool columns, except where the lead verified the handler (rows 7 and 8: `source-verified`).

| # | Message (file:line) | What it does | Min ver | Wrapped | Tools using it |
|---|---|---|---|---|---|
| 1 | GetVersion (base_commands:26) | KiCad version | untagged | yes 1443 | get_editor_state, flip_component |
| 2 | Ping (base:36) | Liveness | untagged | yes 1400/1410 | check_kicad_ui, open_project, IPC failure paths |
| 3 | GetKiCadBinaryPath (base:41) | Path of kicad-cli and others | untagged | no | unused |
| 4 | GetTextExtents (base:54) | Bounding box of a text | untagged | no | unused (P1-50) |
| 5 | GetTextAsShapes (base:71) | Text rendered to strokes and polygons | untagged | no | unused (P1-50) |
| 6 | GetPluginSettingsPath (base:94) | Plugin data directory | untagged | no | unused |
| 7 | GetNetClasses (project_commands:28) | All netclasses | untagged | no | unused; tools read `.kicad_pro` (P1-45) |
| 8 | SetNetClasses (project:38) | Merge or replace netclasses live | untagged | no | unused; `create_netclass` writes the file (P1-45) |
| 9 | ExpandTextVariables (project:54) | Expand `${VAR}` | untagged | no | unused (P1-43) |
| 10 | GetTextVariables (project:69) | Project text variables | untagged | no | unused (P1-43) |
| 11 | SetTextVariables (project:75) | Set text variables | untagged | no | unused (P1-43) |
| 12 | RefreshEditor (editor_commands:33) | Refresh a frame | untagged | no | unused |
| 13 | GetOpenDocuments (editor:39) | List open documents | untagged | yes 1461/1473 | nearly every live tool |
| 14 | SaveDocument (editor:50) | Save | untagged | yes 2465 | save_project, run_drc, get_drc_violations |
| 15 | SaveCopyOfDocument (editor:76) | Save to a path without opening it | untagged | no | unused (P1-52) |
| 16 | RevertDocument (editor:85) | Revert to disk | untagged | no | unused (P1-53) |
| 17 | RunAction (editor:96) | Run a tool action by string ("not an API") | untagged | yes 4655 | no tool; test probe only (`pcb_sync.rs:2548`) |
| 18 | BeginCommit (editor:123) | Start undo group | untagged | yes 2475 | placements, fanout, SES apply, outline, sync, flip, arrays |
| 19 | EndCommit (editor:143) | End undo group | untagged | yes 2491 | same as BeginCommit (`run_commit` 2534) |
| 20 | CreateItems (editor:161) | Create items, optionally in a footprint | untagged | yes 2223/2249 | routing, vias, zones, outline, text, logo, holes, placement, fanout, SES, sync |
| 21 | GetItems (editor:216) | List items by type | untagged | yes 2102/2117/2127 | most PCB reads |
| 22 | GetItemsById (editor:225) | Fetch by KIID | untagged | no | unused (P1-51) |
| 23 | UpdateItems (editor:245) | Update items | untagged | yes 2313/2328 | move, rotate, align, edit_component, footprint graphics, models, library update, repair, sync |
| 24 | DeleteItems (editor:276) | Delete by KIID | untagged | yes 2355/2365 | delete_trace, modify_trace, delete_component, delete_graphics, outline replace, SES rollback |
| 25 | GetBoundingBox (editor:319) | Item bounding boxes | untagged | yes 2052/2074 | get_board_extents, manufacturing package, sync |
| 26 | GetSelection (editor:338) | Read selection (PCB and schematic) | untagged | yes 1514 | get_editor_selection, mutate_editor_selection |
| 27 | AddToSelection (editor:355) | Add to selection | untagged | yes 1569 | mutate_editor_selection |
| 28 | RemoveFromSelection (editor:364) | Remove from selection | untagged | yes 1635-1649 | mutate_editor_selection |
| 29 | ClearSelection (editor:373) | Clear selection | untagged | yes 1635-1649 | mutate_editor_selection |
| 30 | HitTest (editor:379) | Point-in-item test with tolerance | untagged | no | unused (P1-51) |
| 31 | GetTitleBlockInfo (editor:403) | Title block | untagged | yes 2780 | get_board_info (4 of 13 fields) |
| 32 | SetTitleBlockInfo (editor:408) | Write title block | 6-snap (R3: 10.0.1) | no | unused (P1-38) |
| 33 | SaveDocumentToString (editor:415) | Whole board as s-expression | untagged | yes 1921 | DRC sync, specctra, JLCPCB, placement, sync, revision checks |
| 34 | SaveSelectionToString (editor:427) | Selection as s-expression | untagged | no | unused (P1-54) |
| 35 | ParseAndCreateItemsFromString (editor:442) | Paste s-expression | untagged | no (removed: returned OK, created nothing) | unused (P1-54) |
| 36 | GetBoardStackup (board_commands:36) | Physical stackup | untagged | yes 4616/4632 | get_board_stackup |
| 37 | UpdateBoardStackup (board:49) | Write stackup | not implemented upstream | no | unused (section 3) |
| 38 | GetBoardEnabledLayers (board:55) | Copper count and enabled layers | untagged | yes 4469 | get_layer_list, get_board_info |
| 39 | SetBoardEnabledLayers (board:72) | Set copper count and enabled layers; destructive | untagged (R3: 10.0.6) | no | unused (P1-39) |
| 40 | GetGraphicsDefaults (board:85) | Default text and line widths per layer class | untagged | no | unused (P1-44) |
| 41 | GetBoardOrigin (board:103) | Grid or drill origin | untagged | no | unused (P1-42) |
| 42 | SetBoardOrigin (board:110) | Set grid or drill origin | untagged | no | unused (P1-42) |
| 43 | GetBoardLayerName (board:119) | User layer name | untagged | yes 4583 | get_layer_list |
| 44 | GetBoardLayerByName (board:135) | Name to layer ID | 10.0.6 | no | unused (P1-39) |
| 45 | GetNets (board:147) | Nets, optional netclass filter | untagged | yes 1944 (filter unused) | get_nets_list, net resolution, get_board_info, get_netclasses, SES, sync |
| 46 | GetItemsByNet (board:164) | Copper items of given nets | untagged | no | unused (P1-47) |
| 47 | GetItemsByNetClass (board:182) | Copper items of given netclasses | untagged | no | unused (P1-47) |
| 48 | GetConnectedItems (board:197) | Copper-connected items from seeds | 6-snap | no | unused (P1-48) |
| 49 | GetNetClassForNets (board:214) | Effective netclass per net | untagged | yes 1970 | export_specctra_dsn only (P1-46) |
| 50 | RefillZones (board:232) | Refill all or listed zones; async | untagged | yes 2434 (always all) | refill_zones, add_zone, run_drc (P1-14) |
| 51 | GetPadShapeAsPolygon (board:246) | Pad outline per layer | untagged | no | unused (P1-49) |
| 52 | CheckPadstackPresenceOnLayers (board:277) | Is a pad or via flashed on a layer | untagged | no | unused (P1-49) |
| 53 | InjectDrcError (board:321) | Add a DRC marker | 10.0 (enum tag) | no | unused (P2-19) |
| 54 | GetVisibleLayers (board:342) | Layer visibility | untagged | no | unused (P1-41) |
| 55 | SetVisibleLayers (board:357) | Set layer visibility | untagged | no | unused (P1-41) |
| 56 | GetActiveLayer (board:365) | Active layer | untagged | no | unused (P1-40) |
| 57 | SetActiveLayer (board:370) | Set active layer | untagged | no | `set_active_layer` refuses (P1-40) |
| 58 | GetBoardEditorAppearanceSettings (board:429) | Dim, net colours, flip view, ratsnest | untagged | no | unused (P1-41) |
| 59 | SetBoardEditorAppearanceSettings (board:433) | Set appearance | untagged | no | unused (P1-41) |
| 60 | FlipItems (board:446) | Flip items | 10.0.6 | yes 3535/3574 | flip_component |
| 61 | InteractiveMoveItems (board:487) | User-driven move; editor busy until done | untagged | no | unused (P1-55) |

Object types without an item message: `KOT_PCB_TABLE`, `KOT_PCB_TABLECELL`, `KOT_PCB_GENERATOR`, `KOT_PCB_MARKER` (`enums.proto:42-47`). KiCad 10 answers `AS_BAD_REQUEST` for tables and generators (`client.rs:4320-4327`).

## Appendix B. Schematic element coverage (`.kicad_sch`)

From R2 section 1. C, R, U, D: create, read, update, delete. KiCad 10 writes every element listed, confirmed from templates, fixtures, a project file saved by KiCad 10.0.8, or the editor binary's format strings. Status: `code-read` for tool coverage; grammar of elements with no on-disk example is `unverified`.

| Element | C | R | U | D | Gaps | Items |
|---|---|---|---|---|---|---|
| Symbol placement | `add_schematic_component`, `batch_place_components` | list, get, `get_schematic_layout` | move, rotate, `move_connected`, `move_region`, `bulk_move`, `replace_component`, library update | delete, batch | No mirror after placement, no unit change, no body style, no pin alternate, no attribute flags. | P1-57, P1-64, P1-65 |
| Symbol fields | value, footprint, datasheet, custom via edit tools | yes | position, rotation, hide; reset positions | none | No delete; no show_name, size, bold, italic, justify, autoplace flags. | P1-66 |
| Simulation model fields | plain custom fields only | yes | same | none | No `exclude_from_sim`; no model validation. | P1-57, P3-08 |
| Annotation, sheet instances | `annotate_schematic` | `get_sheet_hierarchy` | `renumber_sheet_pages` | none | No clear; no numbering options. | P1-76 |
| Wire | `add_wire`, batch, connection tools, split | `list_schematic_wires` | split only | delete, batch | No stroke settings; endpoints cannot move. | P1-73 |
| Bus | `add_bus`, batch | internal only | none | UUID only | No list or edit. | P1-74 |
| Bus entry | `add_bus_entry`, `connect_pins_to_bus` | internal only | none | UUID only | No list. | P1-74 |
| Bus alias | none | none | none | none | Missing entirely. | P0-38, P7-07 |
| Junction | `add_junction`, batch | internal | none | `batch_delete` | No diameter or colour. | P1-75 |
| No-connect | `add_no_connect`, batch | no lister | follows the pin | delete, batch | No list. | P1-75 |
| Local label | label and connect tools | `list_schematic_labels` (no UUID) | rotate, batch rotate, move by offset | by name and position | No rename, retype, font or size. | P1-68 |
| Global label | same, with `label_type` | same | rotate, move | same | Shape fixed after creation; no intersheet references. | P1-68, P1-79 |
| Hierarchical label | same | same | same | same | Same as global label. | P1-68, P1-69 |
| Netclass directive label | none | not parsed | none | UUID only | Missing, with `Netclass` and `Component Class` fields. | P1-63 |
| Power symbol | `add_power_symbol` | counted as nets | symbol tools | symbol tools | Hard-coded lib ID; no `(power local)`; no `#PWR` renumbering beyond placement. | P1-61, P1-62 |
| Sheet | `add_hierarchical_sheet`, `duplicate_sheet` | `get_sheet_hierarchy` | `edit_sheet`, `move_sheet` | `delete_sheet` | No colours, custom fields, sheet attributes; multiple top-level sheets not handled. | P1-70, P0-07 |
| Sheet pin | `add_sheet_pin`, `import_sheet_pins` | yes, `validate_sheet_pins` | `edit_sheet_pin` | `delete_sheet_pin` | Effectively complete. | none |
| Text | `add_schematic_text` | none | none | UUID only | No list, edit, move; no hyperlink. | P1-60 |
| Text box | none | bounds only | none | UUID only | Missing. | P1-60 |
| Polyline, rectangle, circle, arc, bezier | none | none | none | UUID only | Missing. | P1-58 |
| Image | none | none | none | UUID only | Missing (base64 PNG). | P1-72 |
| Table | none | none | none | UUID only | Missing. | P1-72 |
| Rule area | none | none | none | UUID only | Missing; carries directive labels. | P1-72 |
| Group | none (property only) | none | none | UUID only | Missing. | P0-36, P1-72 |
| Title block | none | none | none | protected | Missing. | P1-56 |
| Page settings | `create_schematic`, `set_schematic_page` | via file | size, orientation | none | No `User` size; drawing sheet is project level. | P1-71, P7-09 |
| Design variants | none | none | none | none | Missing; rejected upstream. | P5-23, P0-33 |
| Embedded files and fonts | none | none | none | protected | Missing. | P1-80 |

## Appendix C. kicad-cli coverage

From R2 section 4 (`kicad-cli --help` of KiCad 10.0.8; argument builders in `T/cli.rs`). Status: `code-read`.

| Subcommand | Tool | Exposed options | Not exposed | Items |
|---|---|---|---|---|
| `sch erc` | `run_erc` | JSON and mm fixed; severity filtered after the run | `-D`, `--severity-exclusions`, `--exit-code-violations` | P5-01, P5-02 |
| `sch upgrade` | none | none | `--force` | P5-19 |
| `sch export bom` | `export_bom` (pcb_export toolset); manufacturing package | fields, labels, group-by, exclude-dnp | `--variant`, `--preset`, `--format-preset`, sort field and order, `--filter`, field, string and ref delimiters, keep tabs, keep line breaks | P5-03 |
| `sch export netlist` | `generate_netlist`, `export_netlist` | format | `--variant` | P5-04, P0-23 |
| `sch export pdf` | `export_schematic_pdf`, `snapshot_project` | black and white, root page | pages, theme, drawing sheet, `-D`, variant, exclude drawing sheet, font, hop-over, three PDF exclusions, `-n` | P5-05 |
| `sch export svg` | `export_schematic_svg`, view and render tools, baselines | black and white, theme | pages, exclude drawing sheet, `-n`, drawing sheet, `-D`, variant, font, hop-over | P5-06 |
| `sch export dxf`, `ps`, `hpgl`, `python-bom` | none | none | all | P5-07 |
| `pcb drc` | `run_drc`, `get_drc_violations` | JSON, schematic parity (always on), refill zones | `--all-track-errors`, `--save-board`, `--severity-exclusions`, `-D`, `--units` | P5-08 |
| `pcb export gerbers` | `export_gerber`, manufacturing package | `--layers` | board plot params, `--cl`, no X2, no netlist, subtract mask, precision, drill origin, aperture macros, Protel ext, check zones, variant, sheet and value flags | P5-09 |
| `pcb export drill` | inside `export_gerber` and the package | separate PTH fixed | Gerber format, map and format, report, zeros, units, oval format, origin, mirror-y, min header, tenting, precision | P5-10 |
| `pcb export pos` | `export_position_file` | CSV or Gerber, side, units, exclude-dnp | ASCII, SMD only, exclude through-hole, bottom negate X, drill origin, board edge, variant | P5-11 |
| `pcb export pdf` | `export_pdf` | layers, black and white, single mode | separate and multipage, mirror, negative, theme, drill shapes, scale, background, `--cl`, variant, sheet | P5-12 |
| `pcb export svg` | `export_svg` | layers, black and white, single mode | page size mode, fit to board, exclude drawing sheet, mirror, negative, theme, drill shapes, multi mode, variant | P5-13 |
| `pcb export dxf` | `export_dxf` | layers, multi mode | units, origin, scale, outline mode and others | P5-14 |
| `pcb export ps` | none | none | all | P5-07 |
| step, stpz, brep, glb, stl, ply, u3d, xao, 3dpdf, vrml | `export_3d` | format, no unspecified | no DNP, variant, board only, no components, component and net filters, include layers, fuse shapes, origin, min distance, substitution, STEP optimise, VRML units and models dir | P5-15 |
| `pcb export ipc2581` | `export_ipc2581` | units, compress | version, precision, BOM columns, BOM revision, variant | P5-16 |
| `pcb export odb` | `export_odb` | units, compression | precision, variant | P5-16 |
| `pcb export gencad` | `export_gencad` | none | its 5 flags | P5-16 |
| `pcb export ipcd356` | not reachable | none | none | P0-23 |
| `pcb export stats` | none | none | JSON report | P5-17 |
| `pcb export hpgl` | none | none | no longer supported in KiCad 10 | none |
| `pcb render` | `get_board_2d_view` | width, height | side, quality, preset, background, zoom, pan, pivot, rotate, perspective, floor, lighting, stackup colours, variant, JPEG | P2-03 |
| `pcb import` | none | none | all formats | P5-18 |
| `pcb upgrade` | none | none | `--force` | P5-19 |
| `sym export svg` | none | none | symbol, theme, hidden pins and fields | P5-20 |
| `sym upgrade` | opt-in test only | none | none | P5-19 |
| `fp export svg` | none | none | footprint, layers, others | P5-20 |
| `fp upgrade` | none | none | none | P5-19 |
| `jobset run` | none | none | file, output, stop on error | P5-21 |
| `version` | `get_installation_info` | `--version` | `--format commit\|about` | P5-22 |
