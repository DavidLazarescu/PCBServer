//! Teardrop settings on pads and vias, over KiCad's IPC API (KiCad 10.0.7+).
//!
//! Served in the `pcb_routing` toolset: teardrops belong to the track-to-pad
//! and track-to-via joints that toolset routes, and a model routing a board
//! already has it loaded. The IPC side is `konnect_ipc::teardrops`.

use crate::mcp::error::ToolErrorKind;
use crate::mcp::protocol::CallToolResult;
use crate::tool;
use crate::tools::{
    get_path, invalid_arg, ipc_target_error_result, opt_str_list, with_bound_board_ipc_classified,
    BoardAccess, BoardBinding, ToolContext, ToolDef,
};
use konnect_ipc::teardrops::{
    kicad_supports_teardrops, IpcTeardropEditOutcome, IpcTeardropItem, IpcTeardropItemKind,
    IpcTeardropPatch, IpcTeardropSelection, IpcTeardropSettings, IpcTeardropSnapshot,
    TeardropSelectionError, TEARDROP_MIN_VERSION,
};
use konnect_ipc::IpcKiCadVersion;
use serde_json::{json, Value};

const DEFAULT_LIMIT: u64 = 500;
const MAX_LIMIT: u64 = 20_000;
/// KiCad keeps lengths in 32-bit nanometres.
const MAX_DISTANCE_MM: f64 = 1000.0;
const NM_PER_MM: f64 = 1_000_000.0;

fn selection_properties() -> serde_json::Map<String, Value> {
    let properties = json!({
        "item_ids":   { "type": "array", "items": { "type": "string" }, "description": "Exact KIIDs of pads or vias (from get_teardrops)" },
        "nets":       { "type": "array", "items": { "type": "string" }, "description": "Only pads and vias on these nets" },
        "references": { "type": "array", "items": { "type": "string" }, "description": "Only pads of these footprints (e.g. U1); leaves vias out" },
        "kinds":      { "type": "array", "items": { "type": "string", "enum": ["pad", "via"] }, "description": "Only these item kinds (default both)" },
        "pad_types":  { "type": "array", "items": { "type": "string", "enum": ["pth", "smd", "edge_connector", "npth"] }, "description": "Only pads of these types; leaves vias out" }
    });
    match properties {
        Value::Object(map) => map,
        _ => unreachable!(),
    }
}

pub fn tools() -> Vec<ToolDef> {
    let mut get_properties = selection_properties();
    get_properties.insert(
        "board".into(),
        json!({ "type": "string", "description": "Path to the .kicad_pcb open in KiCad" }),
    );
    get_properties.insert(
        "limit".into(),
        json!({ "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "default": DEFAULT_LIMIT, "description": "Most items listed; the counts always cover every selected item" }),
    );

    let mut set_properties = selection_properties();
    set_properties.insert(
        "board".into(),
        json!({ "type": "string", "description": "Path to the .kicad_pcb open in KiCad" }),
    );
    for (name, schema) in [
        (
            "all",
            json!({ "type": "boolean", "description": "Address every pad and via on the board. Required when no filter is given; not allowed with one" }),
        ),
        (
            "enabled",
            json!({ "type": "boolean", "description": "Teardrops on (true) or off (false)" }),
        ),
        (
            "curved_edges",
            json!({ "type": "boolean", "description": "Curved instead of straight teardrop sides" }),
        ),
        (
            "allow_multiple_track_segments",
            json!({ "type": "boolean", "description": "Let a teardrop extend over a second track segment when the first is too short" }),
        ),
        (
            "prefer_zone_connection",
            json!({ "type": "boolean", "description": "No teardrop where the pad or via also connects to a copper zone of its net" }),
        ),
        (
            "max_length_mm",
            json!({ "type": "number", "minimum": 0, "maximum": MAX_DISTANCE_MM, "description": "Longest teardrop in mm; 0 means no limit" }),
        ),
        (
            "max_width_mm",
            json!({ "type": "number", "minimum": 0, "maximum": MAX_DISTANCE_MM, "description": "Widest teardrop in mm; 0 means no limit" }),
        ),
        (
            "best_length_ratio",
            json!({ "type": "number", "exclusiveMinimum": 0, "description": "Preferred length as a fraction of the pad or via size (KiCad default 0.5)" }),
        ),
        (
            "best_width_ratio",
            json!({ "type": "number", "exclusiveMinimum": 0, "description": "Preferred width as a fraction of the pad or via size (KiCad default 1.0)" }),
        ),
        (
            "max_track_width_ratio",
            json!({ "type": "number", "minimum": 0, "maximum": 1, "description": "No teardrop when the track is wider than this fraction of the pad or via size (KiCad default 0.9)" }),
        ),
    ] {
        set_properties.insert(name.into(), schema);
    }

    vec![
        tool!(
            "get_teardrops",
            "Read the teardrop settings of pads and vias from the board open in KiCad \
             (KiCad 10.0.7 or newer; older versions are refused with \
             'unsupported_capability'). Filters (item_ids, nets, references, kinds, \
             pad_types) combine: an item must match every one given; none means every \
             pad and via. A filter naming an id, reference or net the board lacks is \
             refused with 'invalid_argument'. Returns per item its KIID, kind, net, \
             footprint reference, pad number and type, and the settings (distances in \
             mm, 0 = no limit); counts of enabled and disabled items; and \
             'teardrop_areas', the teardrop shapes KiCad has built on copper. A pad or \
             via can have teardrops enabled and still have no shape: KiCad builds one \
             only where a track ends on it and the track is narrower than the limits. \
             Board Setup > Teardrops defaults are not read: KiCad's API does not expose \
             them, and they only seed KiCad's own Edit Teardrops dialog.",
            json!({
                "type": "object",
                "properties": Value::Object(get_properties),
                "required": ["board"]
            }),
            |args, ctx| async move { handle_get_teardrops(args, ctx).await }
        )
        .with_board_access(BoardAccess::LiveOnly),
        tool!(
            "set_teardrops",
            "Turn teardrops on or off, or change their shape, on selected pads and vias \
             of the board open in KiCad (KiCad 10.0.7 or newer; older versions are \
             refused with 'unsupported_capability'). Select with filters (item_ids, \
             nets, references, kinds, pad_types; an item must match every one given) \
             or with all: true for every pad and via; a call with neither, or with \
             both, is refused. Only the settings you name change; each item keeps its \
             other values (get_teardrops shows them). All changes go to KiCad in one \
             undo step; pads are written by resending their footprint with only the \
             pad's teardrop changed. KiCad builds the teardrop shapes itself while it \
             applies that step, where a track ends on an enabled pad or via and the \
             track is narrower than the limits; a zone refill (refill_zones) refits \
             them to the copper around them. The result comes from a fresh read: every \
             changed item is checked and every other pad and via must be unchanged, \
             otherwise 'readback_mismatch' or 'mutation_outcome_uncertain' names the \
             items; inspect with get_teardrops before retrying. 'teardrop_areas' gives \
             the shape count before and after. The board is not saved \
             ('persisted': false); call save_project.",
            json!({
                "type": "object",
                "properties": Value::Object(set_properties),
                "required": ["board"]
            }),
            |args, ctx| async move { handle_set_teardrops(args, ctx).await }
        )
        .with_board_access(BoardAccess::LiveOnly),
    ]
}

// ─── Arguments ────────────────────────────────────────────────────────────────

fn parse_selection(args: &Value) -> Result<IpcTeardropSelection, CallToolResult> {
    let list = |key: &str| opt_str_list(args, key).map(Option::unwrap_or_default);
    let kinds = list("kinds")?
        .iter()
        .map(|kind| match kind.as_str() {
            "pad" => Ok(IpcTeardropItemKind::Pad),
            "via" => Ok(IpcTeardropItemKind::Via),
            _ => Err(invalid_arg("kinds", "entries must be 'pad' or 'via'")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let pad_types = list("pad_types")?;
    if let Some(bad) = pad_types
        .iter()
        .find(|t| !["pth", "smd", "edge_connector", "npth"].contains(&t.as_str()))
    {
        return Err(invalid_arg(
            "pad_types",
            &format!("'{bad}' is not one of pth, smd, edge_connector, npth"),
        ));
    }
    let selection = IpcTeardropSelection {
        item_ids: list("item_ids")?,
        nets: list("nets")?,
        references: list("references")?,
        kinds,
        pad_types,
    };
    for (field, values) in [
        ("item_ids", &selection.item_ids),
        ("nets", &selection.nets),
        ("references", &selection.references),
    ] {
        if values.iter().any(|v| v.trim().is_empty()) {
            return Err(invalid_arg(field, "entries must not be empty"));
        }
    }
    Ok(selection)
}

fn opt_bool(args: &Value, key: &str) -> Result<Option<bool>, CallToolResult> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::Bool(value) => Ok(Some(*value)),
        _ => Err(invalid_arg(key, "expected true or false")),
    }
}

fn opt_number(args: &Value, key: &str) -> Result<Option<f64>, CallToolResult> {
    match &args[key] {
        Value::Null => Ok(None),
        value => value
            .as_f64()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or_else(|| invalid_arg(key, "expected a finite number")),
    }
}

fn opt_distance_nm(args: &Value, key: &str) -> Result<Option<i64>, CallToolResult> {
    let Some(mm) = opt_number(args, key)? else {
        return Ok(None);
    };
    if !(0.0..=MAX_DISTANCE_MM).contains(&mm) {
        return Err(invalid_arg(
            key,
            &format!("must be between 0 and {MAX_DISTANCE_MM} mm"),
        ));
    }
    Ok(Some((mm * NM_PER_MM).round() as i64))
}

fn opt_ratio(args: &Value, key: &str, max: Option<f64>) -> Result<Option<f64>, CallToolResult> {
    let Some(value) = opt_number(args, key)? else {
        return Ok(None);
    };
    match max {
        Some(max) if !(0.0..=max).contains(&value) => {
            Err(invalid_arg(key, &format!("must be between 0 and {max}")))
        }
        None if value <= 0.0 => Err(invalid_arg(key, "must be greater than 0")),
        _ => Ok(Some(value)),
    }
}

fn parse_patch(args: &Value) -> Result<IpcTeardropPatch, CallToolResult> {
    let patch = IpcTeardropPatch {
        enabled: opt_bool(args, "enabled")?,
        curved_edges: opt_bool(args, "curved_edges")?,
        allow_multiple_track_segments: opt_bool(args, "allow_multiple_track_segments")?,
        prefer_zone_connection: opt_bool(args, "prefer_zone_connection")?,
        max_length_nm: opt_distance_nm(args, "max_length_mm")?,
        max_width_nm: opt_distance_nm(args, "max_width_mm")?,
        best_length_ratio: opt_ratio(args, "best_length_ratio", None)?,
        best_width_ratio: opt_ratio(args, "best_width_ratio", None)?,
        max_track_width_ratio: opt_ratio(args, "max_track_width_ratio", Some(1.0))?,
    };
    if patch.is_empty() {
        return Err(invalid_arg(
            "enabled",
            "name at least one setting to change (enabled, curved_edges, \
             allow_multiple_track_segments, prefer_zone_connection, max_length_mm, \
             max_width_mm, best_length_ratio, best_width_ratio, max_track_width_ratio)",
        ));
    }
    Ok(patch)
}

/// The selection of a write: filters or `all`, exactly one of them.
fn parse_write_selection(args: &Value) -> Result<IpcTeardropSelection, CallToolResult> {
    let selection = parse_selection(args)?;
    let all = opt_bool(args, "all")?.unwrap_or(false);
    match (all, selection.is_empty()) {
        (true, false) => Err(invalid_arg(
            "all",
            "all: true addresses every pad and via; leave out the filters, or drop all",
        )),
        (false, true) => Err(invalid_arg(
            "all",
            "select pads and vias with item_ids, nets, references, kinds or pad_types, \
             or pass all: true for the whole board",
        )),
        _ => Ok(selection),
    }
}

fn parse_limit(args: &Value) -> Result<usize, CallToolResult> {
    match &args["limit"] {
        Value::Null => Ok(DEFAULT_LIMIT as usize),
        value => match value.as_u64() {
            Some(limit) if (1..=MAX_LIMIT).contains(&limit) => Ok(limit as usize),
            _ => Err(invalid_arg(
                "limit",
                &format!("must be an integer from 1 to {MAX_LIMIT}"),
            )),
        },
    }
}

// ─── Results ──────────────────────────────────────────────────────────────────

fn mm(nm: i64) -> f64 {
    nm as f64 / NM_PER_MM
}

fn settings_json(settings: &IpcTeardropSettings) -> Value {
    json!({
        "enabled": settings.enabled,
        "curved_edges": settings.curved_edges,
        "allow_multiple_track_segments": settings.allow_multiple_track_segments,
        "prefer_zone_connection": settings.prefer_zone_connection,
        "max_length_mm": mm(settings.max_length_nm),
        "max_width_mm": mm(settings.max_width_nm),
        "best_length_ratio": settings.best_length_ratio,
        "best_width_ratio": settings.best_width_ratio,
        "max_track_width_ratio": settings.max_track_width_ratio,
    })
}

fn item_json(item: &IpcTeardropItem) -> Value {
    let mut value = json!({
        "kind": item.kind,
        "id": item.id,
        "net": item.net,
        "teardrop": item.settings.as_ref().map(settings_json),
    });
    if item.kind == IpcTeardropItemKind::Pad {
        value["footprint"] = json!(item.footprint_reference);
        value["pad"] = json!(item.pad_number);
        value["pad_type"] = json!(item.pad_type);
    }
    value
}

fn version_text(version: &IpcKiCadVersion) -> String {
    if version.full_version.is_empty() {
        format!("{}.{}.{}", version.major, version.minor, version.patch)
    } else {
        version.full_version.clone()
    }
}

fn unsupported(version: &IpcKiCadVersion) -> CallToolResult {
    let (major, minor, patch) = TEARDROP_MIN_VERSION;
    let running = version_text(version);
    CallToolResult::error_kind(
        ToolErrorKind::UnsupportedCapability {
            capability: "pad_via_teardrop_settings".into(),
            kicad_version: Some(running.clone()),
        },
        format!(
            "KiCad {running} does not carry teardrop settings on pads and vias over its API; \
             they exist from KiCad {major}.{minor}.{patch}. Nothing was read or changed. \
             Update KiCad, or set teardrops in KiCad with Edit > Edit Teardrops."
        ),
    )
}

fn selection_error(error: &TeardropSelectionError) -> CallToolResult {
    let field = match error {
        TeardropSelectionError::UnknownItemIds(_) => "item_ids",
        TeardropSelectionError::UnknownReferences(_) => "references",
        TeardropSelectionError::UnknownNets(_) => "nets",
        TeardropSelectionError::NothingSelected => "all",
        TeardropSelectionError::SettingsUnavailable(_) => {
            return CallToolResult::error_kind(
                ToolErrorKind::UnsupportedCapability {
                    capability: "pad_via_teardrop_settings".into(),
                    kicad_version: None,
                },
                format!("{error}; nothing was changed."),
            )
        }
    };
    CallToolResult::error_kind(
        ToolErrorKind::InvalidArgument {
            field: field.into(),
            reason: error.to_string(),
        },
        format!("{error}; nothing was changed."),
    )
}

fn handler_error(error: &anyhow::Error) -> CallToolResult {
    if let Some(selection) = error.downcast_ref::<TeardropSelectionError>() {
        return selection_error(selection);
    }
    CallToolResult::error_kind(
        ToolErrorKind::HandlerError {
            reason: format!("{error:#}"),
        },
        format!("Teardrop operation failed: {error:#}"),
    )
}

/// Map a failed binding to the result every live-board tool gives.
fn binding_error<T>(
    binding: Result<BoardBinding<T>, konnect_ipc::IpcFailure>,
) -> Result<T, CallToolResult> {
    match binding {
        Ok(BoardBinding::Bound(value)) => Ok(value),
        Ok(BoardBinding::Unserved { message, .. }) => Err(CallToolResult::error_kind(
            ToolErrorKind::EditorUnavailable {
                editor: "pcb".into(),
                reason: message.clone(),
            },
            format!("KiCad must be running with the requested board open: {message}"),
        )),
        Err(konnect_ipc::IpcFailure::Target { error, .. }) => Err(ipc_target_error_result(&error)),
        Err(error) => Err(CallToolResult::error_kind(
            ToolErrorKind::EditorUnavailable {
                editor: "pcb".into(),
                reason: error.message().to_string(),
            },
            format!("KiCad could not serve the requested live-board operation: {error}"),
        )),
    }
}

enum Live<T> {
    Unsupported(IpcKiCadVersion),
    Done(IpcKiCadVersion, anyhow::Result<T>),
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn handle_get_teardrops(args: &Value, ctx: &ToolContext) -> anyhow::Result<CallToolResult> {
    let board = get_path(args, "board")?;
    let selection = match parse_selection(args) {
        Ok(selection) => selection,
        Err(result) => return Ok(result),
    };
    let limit = match parse_limit(args) {
        Ok(limit) => limit,
        Err(result) => return Ok(result),
    };

    let binding = with_bound_board_ipc_classified(ctx, &board, move |client, document| {
        let version = client.get_kicad_version()?;
        if !kicad_supports_teardrops(&version) {
            return Ok(Live::Unsupported(version));
        }
        Ok(Live::Done(version, client.teardrop_snapshot_in(document)))
    })
    .await?;
    let live = match binding_error(binding) {
        Ok(live) => live,
        Err(result) => return Ok(result),
    };
    let (version, snapshot) = match live {
        Live::Unsupported(version) => return Ok(unsupported(&version)),
        Live::Done(_, Err(error)) => return Ok(handler_error(&error)),
        Live::Done(version, Ok(snapshot)) => (version, snapshot),
    };
    Ok(read_result(&version, &snapshot, &selection, limit))
}

fn read_result(
    version: &IpcKiCadVersion,
    snapshot: &IpcTeardropSnapshot,
    selection: &IpcTeardropSelection,
    limit: usize,
) -> CallToolResult {
    let indices = match selection.select(&snapshot.items) {
        Ok(indices) => indices,
        // A read of a board without pads and vias, or of filters that exclude
        // each other, is a valid empty answer.
        Err(TeardropSelectionError::NothingSelected) => Vec::new(),
        Err(error) => return selection_error(&error),
    };
    let selected: Vec<&IpcTeardropItem> = indices.iter().map(|&i| &snapshot.items[i]).collect();
    let count = |kind: IpcTeardropItemKind, enabled: Option<bool>| {
        selected
            .iter()
            .filter(|item| item.kind == kind)
            .filter(|item| {
                enabled.is_none_or(|on| item.settings.as_ref().is_some_and(|s| s.enabled == on))
            })
            .count()
    };
    let without_settings: Vec<&str> = selected
        .iter()
        .filter(|item| item.settings.is_none())
        .map(|item| item.id.as_str())
        .collect();
    let mut body = json!({
        "source": "ipc",
        "kicad_version": version_text(version),
        "counts": {
            "pads": count(IpcTeardropItemKind::Pad, None),
            "pads_enabled": count(IpcTeardropItemKind::Pad, Some(true)),
            "vias": count(IpcTeardropItemKind::Via, None),
            "vias_enabled": count(IpcTeardropItemKind::Via, Some(true)),
        },
        "teardrop_areas": snapshot.areas,
        "items": selected.iter().take(limit).map(|item| item_json(item)).collect::<Vec<_>>(),
        "truncated": selected.len() > limit,
    });
    if !without_settings.is_empty() {
        body["items_without_settings"] = json!(without_settings);
    }
    CallToolResult::json(&body)
}

async fn handle_set_teardrops(args: &Value, ctx: &ToolContext) -> anyhow::Result<CallToolResult> {
    let board = get_path(args, "board")?;
    let selection = match parse_write_selection(args) {
        Ok(selection) => selection,
        Err(result) => return Ok(result),
    };
    let patch = match parse_patch(args) {
        Ok(patch) => patch,
        Err(result) => return Ok(result),
    };

    let binding = with_bound_board_ipc_classified(ctx, &board, move |client, document| {
        let version = client.get_kicad_version()?;
        if !kicad_supports_teardrops(&version) {
            return Ok(Live::Unsupported(version));
        }
        Ok(Live::Done(
            version,
            client.set_teardrops_in(document, &selection, &patch),
        ))
    })
    .await?;
    let live = match binding_error(binding) {
        Ok(live) => live,
        Err(result) => return Ok(result),
    };
    let (version, outcome) = match live {
        Live::Unsupported(version) => return Ok(unsupported(&version)),
        Live::Done(_, Err(error)) => return Ok(handler_error(&error)),
        Live::Done(version, Ok(outcome)) => (version, outcome),
    };
    Ok(write_result(&board, &version, outcome))
}

/// How many ids a result lists by name before it only counts them.
const LISTED_IDS: usize = 50;

fn write_result(
    board: &std::path::Path,
    version: &IpcKiCadVersion,
    outcome: IpcTeardropEditOutcome,
) -> CallToolResult {
    match outcome {
        IpcTeardropEditOutcome::Applied {
            selected,
            changed,
            before,
            after,
        } => {
            let mut body = json!({
                "source": "ipc",
                "kicad_version": version_text(version),
                "changed": !changed.is_empty(),
                "selected_count": selected.len(),
                "changed_count": changed.len(),
                "changed_ids": changed.iter().take(LISTED_IDS).collect::<Vec<_>>(),
                "unchanged_count": selected.len() - changed.len(),
                "teardrop_areas": { "before": before.areas, "after": after.areas },
                "persisted": false,
            });
            let enabled_now = after
                .items
                .iter()
                .filter(|item| changed.contains(&item.id))
                .any(|item| item.settings.as_ref().is_some_and(|s| s.enabled));
            if enabled_now && after.areas == before.areas {
                body["teardrops_note"] = json!(
                    "Teardrops are enabled on the changed items, but KiCad built no new \
                     teardrop shape. KiCad builds one only where a track ends on the pad or \
                     via and the track is narrower than max_width_mm and \
                     max_track_width_ratio allow; prefer_zone_connection also skips pads \
                     inside a zone of their net. A board that still uses legacy teardrops \
                     gets none until it is converted in KiCad."
                );
            }
            CallToolResult::json(&body)
        }
        IpcTeardropEditOutcome::Uncertain {
            changed,
            mismatches,
            accepted,
            reason,
            ..
        } => {
            let named: Vec<String> = mismatches
                .iter()
                .take(LISTED_IDS)
                .map(|m| {
                    format!(
                        "{} (expected {}, read {})",
                        m.id,
                        m.expected
                            .as_ref()
                            .map_or("nothing".into(), |s| settings_json(s).to_string()),
                        m.observed
                            .as_ref()
                            .map_or("nothing".into(), |s| settings_json(s).to_string()),
                    )
                })
                .collect();
            let detail = if named.is_empty() {
                String::new()
            } else {
                format!(" Items that differ: {}.", named.join("; "))
            };
            if accepted && !mismatches.is_empty() {
                let wrong: std::collections::BTreeSet<&str> =
                    mismatches.iter().map(|m| m.id.as_str()).collect();
                return CallToolResult::error_kind(
                    ToolErrorKind::ReadbackMismatch {
                        operation: "set_teardrops".into(),
                        requested_kiids: changed.clone(),
                        before_kiids: Vec::new(),
                        after_kiids: changed
                            .iter()
                            .filter(|id| !wrong.contains(id.as_str()))
                            .cloned()
                            .collect(),
                    },
                    format!(
                        "{reason}. 'after_kiids' lists the requested items that read back as \
                         requested. Inspect with get_teardrops before retrying.{detail}"
                    ),
                );
            }
            CallToolResult::error_kind(
                ToolErrorKind::MutationOutcomeUncertain {
                    operation: "set_teardrops".into(),
                    path: board.display().to_string(),
                    reason: reason.clone(),
                },
                format!(
                    "KiCad may have changed teardrop settings on {}, but a fresh read did not \
                     prove the requested state. Inspect with get_teardrops before retrying. \
                     {reason}.{detail}",
                    board.display()
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal_field(result: CallToolResult) -> String {
        assert!(result.is_error);
        let text = match &result.content[0] {
            crate::mcp::protocol::ToolContent::Text { text } => text.clone(),
            other => panic!("unexpected content {other:?}"),
        };
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(body["error"]["kind"], "invalid_argument", "{body}");
        body["error"]["field"].as_str().unwrap().to_string()
    }

    #[test]
    fn a_write_needs_filters_or_all_but_not_both() {
        assert_eq!(
            refusal_field(parse_write_selection(&json!({})).unwrap_err()),
            "all"
        );
        assert_eq!(
            refusal_field(
                parse_write_selection(&json!({ "all": true, "nets": ["GND"] })).unwrap_err()
            ),
            "all"
        );
        assert!(parse_write_selection(&json!({ "all": true }))
            .unwrap()
            .is_empty());
        let nets = parse_write_selection(&json!({ "nets": ["GND"] })).unwrap();
        assert_eq!(nets.nets, vec!["GND".to_string()]);
    }

    #[test]
    fn selection_entries_are_checked() {
        assert_eq!(
            refusal_field(parse_selection(&json!({ "kinds": ["track"] })).unwrap_err()),
            "kinds"
        );
        assert_eq!(
            refusal_field(parse_selection(&json!({ "pad_types": ["thru"] })).unwrap_err()),
            "pad_types"
        );
        assert_eq!(
            refusal_field(parse_selection(&json!({ "nets": [""] })).unwrap_err()),
            "nets"
        );
        assert_eq!(
            refusal_field(parse_selection(&json!({ "item_ids": "abc" })).unwrap_err()),
            "item_ids"
        );
    }

    #[test]
    fn a_write_must_name_a_setting() {
        assert_eq!(
            refusal_field(parse_patch(&json!({})).unwrap_err()),
            "enabled"
        );
    }

    #[test]
    fn settings_are_converted_and_bounded() {
        let patch = parse_patch(&json!({
            "enabled": true,
            "max_length_mm": 1.25,
            "max_width_mm": 0,
            "best_width_ratio": 1.5,
            "max_track_width_ratio": 0.8
        }))
        .unwrap();
        assert_eq!(patch.enabled, Some(true));
        assert_eq!(patch.max_length_nm, Some(1_250_000));
        assert_eq!(patch.max_width_nm, Some(0));
        assert_eq!(patch.best_width_ratio, Some(1.5));
        assert_eq!(patch.max_track_width_ratio, Some(0.8));
        assert_eq!(patch.curved_edges, None);

        assert_eq!(
            refusal_field(parse_patch(&json!({ "max_length_mm": -1 })).unwrap_err()),
            "max_length_mm"
        );
        assert_eq!(
            refusal_field(parse_patch(&json!({ "max_width_mm": 5000 })).unwrap_err()),
            "max_width_mm"
        );
        assert_eq!(
            refusal_field(parse_patch(&json!({ "best_length_ratio": 0 })).unwrap_err()),
            "best_length_ratio"
        );
        assert_eq!(
            refusal_field(parse_patch(&json!({ "max_track_width_ratio": 1.5 })).unwrap_err()),
            "max_track_width_ratio"
        );
        assert_eq!(
            refusal_field(parse_patch(&json!({ "enabled": "yes" })).unwrap_err()),
            "enabled"
        );
    }

    #[test]
    fn limit_is_bounded() {
        assert_eq!(parse_limit(&json!({})).unwrap(), DEFAULT_LIMIT as usize);
        assert_eq!(parse_limit(&json!({ "limit": 3 })).unwrap(), 3);
        assert_eq!(
            refusal_field(parse_limit(&json!({ "limit": 0 })).unwrap_err()),
            "limit"
        );
        assert_eq!(
            refusal_field(parse_limit(&json!({ "limit": 1.5 })).unwrap_err()),
            "limit"
        );
    }
}
