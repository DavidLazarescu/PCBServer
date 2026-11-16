//! `get_teardrops` and `set_teardrops` through served `tools/call`, against a
//! KiCad double that keeps footprints, vias and zones.
//!
//! The double applies `UpdateItems` the way KiCad 10.0.8's
//! `handleCreateUpdateItemsInternal` does: an item is replaced by the message,
//! so a field the message lacks is lost. When a commit enables teardrops on an
//! item it adds one teardrop zone, standing in for `BOARD_COMMIT::Push`
//! rebuilding them.

use crate::mcp::handler::McpHandler;
use crate::mcp::protocol::{CallToolResult, ToolContent};
use crate::test_support::MockIpcServer;
use crate::tools::pcb_board::board_mock::board_document_in_project;
use crate::tools::ServerConfig;
use konnect_ipc::builders::{any_is, pack_any};
use konnect_ipc::gen::kiapi;
use konnect_ipc::gen::kiapi::board::types::{
    FootprintInstance, Pad, PadTeardropMode, PadTeardropSettings, Via, Zone,
};
use konnect_ipc::gen::kiapi::common::types::{Distance, Kiid};
use prost::Message;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../konnect-sexp/tests/fixtures/placement"
);

const PAD: &str = "kiapi.board.types.Pad";
const VIA: &str = "kiapi.board.types.Via";
const FOOTPRINT: &str = "kiapi.board.types.FootprintInstance";
const ZONE: &str = "kiapi.board.types.Zone";

struct Project {
    _dir: tempfile::TempDir,
    board: PathBuf,
}

fn project() -> Project {
    let dir = tempfile::tempdir().unwrap();
    for extension in ["kicad_pcb", "kicad_pro"] {
        std::fs::copy(
            Path::new(FIXTURE_DIR).join(format!("placement_fixture.{extension}")),
            dir.path().join(format!("placement_fixture.{extension}")),
        )
        .unwrap();
    }
    Project {
        board: dir.path().join("placement_fixture.kicad_pcb"),
        _dir: dir,
    }
}

// ─── The KiCad double ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Update {
    /// Replace each item by the message, as KiCad does.
    Apply,
    /// Answer every item `ISC_OK` and change nothing: a readback must catch it.
    AcceptAndIgnore,
}

struct Kicad {
    version: (u32, u32, u32),
    footprints: Vec<FootprintInstance>,
    vias: Vec<Via>,
    zones: Vec<Zone>,
    update: Update,
    commands: Vec<String>,
    updates: Vec<kiapi::common::commands::UpdateItems>,
    staged: Vec<prost_types::Any>,
}

fn kiid(value: &str) -> Option<Kiid> {
    Some(Kiid {
        value: value.into(),
    })
}

fn teardrop(enabled: bool) -> PadTeardropSettings {
    PadTeardropSettings {
        mode: if enabled {
            PadTeardropMode::PtmEnabled
        } else {
            PadTeardropMode::PtmDisabled
        } as i32,
        curved_edges: false,
        allow_multiple_track_segments: true,
        prefer_zone_connection: true,
        max_length: Some(Distance {
            value_nm: 1_000_000,
        }),
        max_width: Some(Distance {
            value_nm: 2_000_000,
        }),
        best_length_ratio: 0.5,
        best_width_ratio: 1.0,
        max_track_width_ratio: 0.9,
    }
}

fn net(name: &str, code: i32) -> Option<kiapi::board::types::Net> {
    Some(kiapi::board::types::Net {
        code: Some(kiapi::board::types::NetCode { value: code }),
        name: name.into(),
    })
}

/// A pad as `PAD::Serialize` sends it, with fields beside `teardrop` set so a
/// write that loses one shows.
fn pad(id: &str, number: &str, net_name: &str, settings: Option<PadTeardropSettings>) -> Pad {
    Pad {
        id: kiid(id),
        number: number.into(),
        net: net(net_name, if net_name == "GND" { 1 } else { 2 }),
        r#type: kiapi::board::types::PadType::PtSmd as i32,
        position: Some(kiapi::common::types::Vector2 {
            x_nm: 1_000_000,
            y_nm: 2_000_000,
        }),
        copper_clearance_override: Some(Distance { value_nm: 150_000 }),
        pad_to_die_length: Some(Distance { value_nm: 42 }),
        fab_property: kiapi::board::types::PadFabricationProperty::PfpTestpoint as i32,
        teardrop: settings,
        ..Default::default()
    }
}

fn footprint(id: &str, reference: &str, pads: Vec<Pad>) -> FootprintInstance {
    let field = |text: &str| kiapi::board::types::Field {
        text: Some(kiapi::board::types::BoardText {
            text: Some(kiapi::common::types::Text {
                text: text.into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut items: Vec<prost_types::Any> = pads.iter().map(|p| pack_any(p, PAD)).collect();
    items.push(prost_types::Any {
        type_url: "type.googleapis.com/kiapi.board.types.BoardGraphicShape".into(),
        value: vec![0x0a, 0x03, 0x0a, 0x01, 0x78],
    });
    FootprintInstance {
        id: kiid(id),
        position: Some(kiapi::common::types::Vector2 {
            x_nm: 5_000_000,
            y_nm: 6_000_000,
        }),
        reference_field: Some(field(reference)),
        value_field: Some(field("10k")),
        definition: Some(kiapi::board::types::Footprint {
            items,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn via(id: &str, net_name: &str, settings: Option<PadTeardropSettings>) -> Via {
    Via {
        id: kiid(id),
        position: Some(kiapi::common::types::Vector2 {
            x_nm: 9_000_000,
            y_nm: 9_000_000,
        }),
        net: net(net_name, 1),
        is_free: true,
        teardrop: settings,
        ..Default::default()
    }
}

fn teardrop_zone(id: &str) -> Zone {
    Zone {
        id: kiid(id),
        r#type: kiapi::board::types::ZoneType::ZtTeardrop as i32,
        layers: vec![kiapi::board::types::BoardLayer::BlFCu as i32],
        settings: Some(kiapi::board::types::zone::Settings::CopperSettings(
            kiapi::board::types::CopperZoneSettings {
                teardrop: Some(kiapi::board::types::ZoneTeardropSettings {
                    r#type: kiapi::board::types::ZoneTeardropType::ZtdtViaPad as i32,
                }),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

/// R1 with a GND and a VCC pad, a GND via, and one teardrop zone already built.
/// Teardrops are off everywhere unless `settings` says otherwise.
fn board_state(version: (u32, u32, u32), settings: Option<PadTeardropSettings>) -> Kicad {
    Kicad {
        version,
        footprints: vec![footprint(
            "fp-r1",
            "R1",
            vec![
                pad("pad-1", "1", "GND", settings),
                pad("pad-2", "2", "VCC", settings),
            ],
        )],
        vias: vec![via("via-1", "GND", settings)],
        zones: vec![teardrop_zone("td-0")],
        update: Update::Apply,
        commands: Vec::new(),
        updates: Vec::new(),
        staged: Vec::new(),
    }
}

fn enables_teardrops(item: &prost_types::Any) -> bool {
    let on = |s: &Option<PadTeardropSettings>| {
        s.as_ref()
            .is_some_and(|s| s.mode == PadTeardropMode::PtmEnabled as i32)
    };
    if any_is(item, VIA) {
        return on(&Via::decode(item.value.as_slice()).unwrap().teardrop);
    }
    let footprint = FootprintInstance::decode(item.value.as_slice()).unwrap();
    footprint
        .definition
        .iter()
        .flat_map(|d| d.items.iter())
        .filter(|child| any_is(child, PAD))
        .any(|child| on(&Pad::decode(child.value.as_slice()).unwrap().teardrop))
}

fn apply(state: &mut Kicad) {
    let staged = std::mem::take(&mut state.staged);
    for item in &staged {
        if any_is(item, VIA) {
            let new = Via::decode(item.value.as_slice()).unwrap();
            let slot = state.vias.iter_mut().find(|v| v.id == new.id).unwrap();
            *slot = new;
        } else {
            let new = FootprintInstance::decode(item.value.as_slice()).unwrap();
            let slot = state
                .footprints
                .iter_mut()
                .find(|f| f.id == new.id)
                .unwrap();
            *slot = new;
        }
        if enables_teardrops(item) {
            let id = format!("td-{}", state.zones.len());
            state.zones.push(teardrop_zone(&id));
        }
    }
}

fn spawn_kicad(
    document: kiapi::common::types::DocumentSpecifier,
    state: Kicad,
) -> (MockIpcServer, Arc<Mutex<Kicad>>) {
    let state = Arc::new(Mutex::new(state));
    let shared = state.clone();
    let server = MockIpcServer::spawn("teardrops", move |request| {
        let command = request.message.expect("a command");
        let name = command.type_url.rsplit('.').next().unwrap().to_string();
        let mut state = shared.lock().unwrap();
        state.commands.push(name.clone());
        let ok = |message| kiapi::common::ApiResponse {
            status: Some(kiapi::common::ApiResponseStatus {
                status: kiapi::common::ApiStatusCode::AsOk as i32,
                error_message: String::new(),
            }),
            header: None,
            message,
        };
        match name.as_str() {
            "GetOpenDocuments" => ok(Some(pack_any(
                &kiapi::common::commands::GetOpenDocumentsResponse {
                    documents: vec![document.clone()],
                },
                "kiapi.common.commands.GetOpenDocumentsResponse",
            ))),
            "GetVersion" => {
                let (major, minor, patch) = state.version;
                ok(Some(pack_any(
                    &kiapi::common::commands::GetVersionResponse {
                        version: Some(kiapi::common::types::KiCadVersion {
                            major,
                            minor,
                            patch,
                            full_version: format!("{major}.{minor}.{patch}"),
                        }),
                    },
                    "kiapi.common.commands.GetVersionResponse",
                )))
            }
            "GetItems" => {
                let request =
                    kiapi::common::commands::GetItems::decode(command.value.as_slice()).unwrap();
                assert_eq!(request.header.unwrap().document, Some(document.clone()));
                use kiapi::common::types::KiCadObjectType as T;
                let mut items = Vec::new();
                for kind in request.types {
                    match T::try_from(kind).unwrap() {
                        T::KotPcbFootprint => {
                            items.extend(state.footprints.iter().map(|f| pack_any(f, FOOTPRINT)))
                        }
                        T::KotPcbVia => items.extend(state.vias.iter().map(|v| pack_any(v, VIA))),
                        T::KotPcbZone => {
                            items.extend(state.zones.iter().map(|z| pack_any(z, ZONE)))
                        }
                        other => panic!("asked for {other:?}"),
                    }
                }
                ok(Some(pack_any(
                    &kiapi::common::commands::GetItemsResponse {
                        header: None,
                        status: kiapi::common::types::ItemRequestStatus::IrsOk as i32,
                        items,
                    },
                    "kiapi.common.commands.GetItemsResponse",
                )))
            }
            "BeginCommit" => ok(Some(pack_any(
                &kiapi::common::commands::BeginCommitResponse {
                    id: kiid("commit-1"),
                },
                "kiapi.common.commands.BeginCommitResponse",
            ))),
            "UpdateItems" => {
                let request =
                    kiapi::common::commands::UpdateItems::decode(command.value.as_slice()).unwrap();
                assert_eq!(
                    request.header.as_ref().unwrap().document,
                    Some(document.clone())
                );
                state.updates.push(request.clone());
                if state.update == Update::Apply {
                    state.staged.extend(request.items.iter().cloned());
                }
                ok(Some(pack_any(
                    &kiapi::common::commands::UpdateItemsResponse {
                        header: None,
                        status: kiapi::common::types::ItemRequestStatus::IrsOk as i32,
                        updated_items: request
                            .items
                            .into_iter()
                            .map(|item| kiapi::common::commands::ItemUpdateResult {
                                status: Some(kiapi::common::commands::ItemStatus {
                                    code: kiapi::common::commands::ItemStatusCode::IscOk as i32,
                                    error_message: String::new(),
                                }),
                                item: Some(item),
                            })
                            .collect(),
                    },
                    "kiapi.common.commands.UpdateItemsResponse",
                )))
            }
            "EndCommit" => {
                let request =
                    kiapi::common::commands::EndCommit::decode(command.value.as_slice()).unwrap();
                if request.action == kiapi::common::commands::CommitAction::CmaCommit as i32 {
                    apply(&mut state);
                } else {
                    state.staged.clear();
                }
                ok(Some(pack_any(
                    &kiapi::common::commands::EndCommitResponse {},
                    "kiapi.common.commands.EndCommitResponse",
                )))
            }
            other => panic!("the double was sent {other}"),
        }
    });
    (server, state)
}

// ─── Served calls ────────────────────────────────────────────────────────────

async fn call(
    server: &MockIpcServer,
    tool: &str,
    board: &Path,
    arguments: Value,
) -> CallToolResult {
    let handler = McpHandler::new(ServerConfig {
        kicad_cli: String::new(),
        kicad_binary: String::new(),
        ipc_address: server.address().to_string(),
        project_dir: None,
        jlcpcb_db_path: None,
        auto_load_toolsets: true,
        eager_toolsets: true,
    })
    .await
    .expect("handler builds");
    let mut arguments = arguments;
    arguments["board"] = json!(board.to_string_lossy());
    let response = handler
        .handle_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        }))
        .await
        .expect("tools/call receives a response");
    assert!(response.error.is_none(), "tool errors are MCP results");
    serde_json::from_value(response.result.expect("a result"))
        .expect("result uses the advertised MCP shape")
}

fn body(result: &CallToolResult) -> Value {
    let Some(ToolContent::Text { text }) = result.content.first() else {
        panic!("expected text content");
    };
    serde_json::from_str(text).unwrap_or_else(|_| json!({ "text": text }))
}

fn kind(result: &CallToolResult) -> Option<String> {
    crate::mcp::error::extract_error_kind(result)
}

fn holding(project: &Project, state: Kicad) -> (MockIpcServer, Arc<Mutex<Kicad>>) {
    spawn_kicad(board_document_in_project(&project.board), state)
}

fn wrote(state: &Kicad) -> bool {
    state
        .commands
        .iter()
        .any(|c| c == "BeginCommit" || c == "UpdateItems")
}

// ─── Reads ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_teardrops_lists_pads_and_vias_with_their_settings() {
    let project = project();
    let (server, _) = holding(&project, board_state((10, 0, 8), Some(teardrop(false))));
    let result = call(&server, "get_teardrops", &project.board, json!({})).await;
    assert!(!result.is_error, "{:?}", body(&result));
    let body = body(&result);
    assert_eq!(body["source"], "ipc");
    assert_eq!(body["counts"]["pads"], 2);
    assert_eq!(body["counts"]["vias"], 1);
    assert_eq!(body["counts"]["pads_enabled"], 0);
    assert_eq!(body["teardrop_areas"]["pad_or_via"], 1);
    let first = &body["items"][0];
    assert_eq!(first["kind"], "pad");
    assert_eq!(first["footprint"], "R1");
    assert_eq!(first["pad"], "1");
    assert_eq!(first["pad_type"], "smd");
    assert_eq!(first["teardrop"]["max_width_mm"], 2.0);
    assert_eq!(first["teardrop"]["max_track_width_ratio"], 0.9);
    assert_eq!(body["items"][2]["kind"], "via");
    assert_eq!(body["truncated"], false);
}

#[tokio::test]
async fn get_teardrops_filters_and_refuses_unknown_names() {
    let project = project();
    let (server, _) = holding(&project, board_state((10, 0, 8), Some(teardrop(false))));
    let gnd = call(
        &server,
        "get_teardrops",
        &project.board,
        json!({ "nets": ["GND"], "kinds": ["pad"] }),
    )
    .await;
    let gnd = body(&gnd);
    assert_eq!(gnd["items"].as_array().unwrap().len(), 1);
    assert_eq!(gnd["items"][0]["id"], "pad-1");

    let typo = call(
        &server,
        "get_teardrops",
        &project.board,
        json!({ "nets": ["GDN"] }),
    )
    .await;
    assert_eq!(kind(&typo).as_deref(), Some("invalid_argument"));
    assert_eq!(body(&typo)["error"]["field"], "nets");
}

#[tokio::test]
async fn kicad_before_10_0_7_is_refused_before_anything_is_read() {
    let project = project();
    let (server, state) = holding(&project, board_state((10, 0, 6), None));
    for (tool, arguments) in [
        ("get_teardrops", json!({})),
        ("set_teardrops", json!({ "all": true, "enabled": true })),
    ] {
        let result = call(&server, tool, &project.board, arguments).await;
        assert_eq!(
            kind(&result).as_deref(),
            Some("unsupported_capability"),
            "{tool}"
        );
        let body = body(&result);
        assert_eq!(body["error"]["kicad_version"], "10.0.6");
        assert!(body["message"].as_str().unwrap().contains("10.0.7"));
    }
    let state = state.lock().unwrap();
    assert!(
        !state.commands.iter().any(|c| c == "GetItems") && !wrote(&state),
        "{:?}",
        state.commands
    );
}

// ─── Writes ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn set_teardrops_changes_only_the_teardrop_of_the_selected_items() {
    let project = project();
    let (server, state) = holding(&project, board_state((10, 0, 8), Some(teardrop(false))));
    let before = {
        let state = state.lock().unwrap();
        (state.footprints.clone(), state.vias.clone())
    };
    let result = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "nets": ["GND"], "enabled": true, "curved_edges": true }),
    )
    .await;
    assert!(!result.is_error, "{:?}", body(&result));
    let response = body(&result);
    assert_eq!(response["changed"], true);
    assert_eq!(response["selected_count"], 2);
    assert_eq!(response["changed_count"], 2);
    assert_eq!(response["teardrop_areas"]["before"]["pad_or_via"], 1);
    assert_eq!(response["teardrop_areas"]["after"]["pad_or_via"], 3);
    assert_eq!(response["persisted"], false);

    let state = state.lock().unwrap();
    assert_eq!(state.updates.len(), 1, "one UpdateItems inside one commit");
    assert_eq!(
        state
            .commands
            .iter()
            .filter(|c| *c == "BeginCommit")
            .count(),
        1
    );
    let mut wanted = teardrop(false);
    wanted.mode = PadTeardropMode::PtmEnabled as i32;
    wanted.curved_edges = true;

    // The footprint differs only in pad 1's teardrop.
    let mut expected_footprint = before.0[0].clone();
    let items = &mut expected_footprint.definition.as_mut().unwrap().items;
    let mut pad_1 = Pad::decode(items[0].value.as_slice()).unwrap();
    pad_1.teardrop = Some(wanted);
    items[0] = pack_any(&pad_1, PAD);
    assert_eq!(state.footprints[0], expected_footprint);
    // The via likewise.
    let mut expected_via = before.1[0].clone();
    expected_via.teardrop = Some(wanted);
    assert_eq!(state.vias[0], expected_via);
}

#[tokio::test]
async fn set_teardrops_by_reference_resends_the_footprint_and_leaves_vias() {
    let project = project();
    let (server, state) = holding(&project, board_state((10, 0, 8), Some(teardrop(false))));
    let result = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "references": ["R1"], "max_width_mm": 1.5 }),
    )
    .await;
    assert!(!result.is_error, "{:?}", body(&result));
    let state = state.lock().unwrap();
    let sent = &state.updates[0].items;
    assert_eq!(sent.len(), 1);
    assert!(any_is(&sent[0], FOOTPRINT));
    assert_eq!(state.vias[0].teardrop, Some(teardrop(false)));
}

#[tokio::test]
async fn set_teardrops_that_changes_nothing_sends_no_commit() {
    let project = project();
    let (server, state) = holding(&project, board_state((10, 0, 8), Some(teardrop(true))));
    let result = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "all": true, "enabled": true }),
    )
    .await;
    assert!(!result.is_error, "{:?}", body(&result));
    assert_eq!(body(&result)["changed"], false);
    assert_eq!(body(&result)["unchanged_count"], 3);
    assert!(!wrote(&state.lock().unwrap()));
}

#[tokio::test]
async fn set_teardrops_reports_a_readback_mismatch() {
    let project = project();
    let mut kicad = board_state((10, 0, 8), Some(teardrop(false)));
    kicad.update = Update::AcceptAndIgnore;
    let (server, _) = holding(&project, kicad);
    let result = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "item_ids": ["via-1"], "enabled": true }),
    )
    .await;
    assert_eq!(kind(&result).as_deref(), Some("readback_mismatch"));
    let body = body(&result);
    assert_eq!(body["error"]["requested_kiids"], json!(["via-1"]));
    assert_eq!(body["error"]["after_kiids"], json!([]));
    assert!(body["message"].as_str().unwrap().contains("via-1"));
}

#[tokio::test]
async fn set_teardrops_refuses_bad_selections_without_writing() {
    let project = project();
    let (server, state) = holding(&project, board_state((10, 0, 8), Some(teardrop(false))));
    for (arguments, field) in [
        (
            json!({ "references": ["U9"], "enabled": true }),
            "references",
        ),
        (json!({ "item_ids": ["nope"], "enabled": true }), "item_ids"),
        (
            json!({ "nets": ["VCC"], "kinds": ["via"], "enabled": true }),
            "all",
        ),
        (json!({ "enabled": true }), "all"),
        (json!({ "all": true }), "enabled"),
    ] {
        let result = call(&server, "set_teardrops", &project.board, arguments.clone()).await;
        assert_eq!(
            kind(&result).as_deref(),
            Some("invalid_argument"),
            "{arguments}"
        );
        assert_eq!(body(&result)["error"]["field"], field, "{arguments}");
    }
    let typo = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "all": true, "enable": true }),
    )
    .await;
    assert!(typo.is_error, "closed schema refuses an unknown field");
    assert!(!wrote(&state.lock().unwrap()));
}

#[tokio::test]
async fn set_teardrops_refuses_items_without_settings() {
    let project = project();
    // A 10.0.7+ version string with messages lacking the field: never guessed.
    let (server, state) = holding(&project, board_state((10, 0, 8), None));
    let result = call(
        &server,
        "set_teardrops",
        &project.board,
        json!({ "all": true, "enabled": true }),
    )
    .await;
    assert_eq!(kind(&result).as_deref(), Some("unsupported_capability"));
    assert!(!wrote(&state.lock().unwrap()));
}
