//! Net class tools while KiCad holds the board: through KiCad's IPC API,
//! never through the project file underneath it.
//!
//! Driven through served `tools/call`, so schema, dispatch and handler are all
//! under test. The KiCad double keeps net classes the way KiCad 10.0's
//! `handleSetNetClasses` and `NETCLASS::Deserialize` do: a merge by class
//! name, values replaced only where the message carries them, the priority
//! always taken from the message. The project is the KiCad-written placement
//! fixture (`konnect-sexp/tests/fixtures/placement/`).

use crate::mcp::handler::McpHandler;
use crate::mcp::protocol::{CallToolResult, ToolContent};
use crate::test_support::MockIpcServer;
use crate::tools::pcb_board::board_mock::{board_document, board_document_in_project};
use crate::tools::ServerConfig;
use konnect_ipc::gen::kiapi;
use konnect_ipc::gen::kiapi::common::project::{NetClass, NetClassBoardSettings};
use konnect_ipc::gen::kiapi::common::types::Distance;
use prost::Message;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../konnect-sexp/tests/fixtures/placement"
);

struct Project {
    _dir: tempfile::TempDir,
    board: PathBuf,
    project_file: PathBuf,
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
        project_file: dir.path().join("placement_fixture.kicad_pro"),
        _dir: dir,
    }
}

/// Both files byte for byte, to prove a call left them alone.
fn snapshot(project: &Project) -> (Vec<u8>, Vec<u8>) {
    (
        std::fs::read(&project.board).unwrap(),
        std::fs::read(&project.project_file).unwrap(),
    )
}

// ─── The KiCad double ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum SetBehaviour {
    /// Apply the message as KiCad 10.0 does.
    Apply,
    /// Answer `AS_OK` and change nothing: a readback must catch this.
    AcceptAndIgnore,
    /// Answer `AS_BAD_REQUEST` and change nothing.
    Refuse,
}

#[derive(Default)]
struct Kicad {
    classes: Vec<NetClass>,
    /// The name of every command received, in order.
    commands: Vec<String>,
    /// Every `SetNetClasses` request received.
    sets: Vec<kiapi::common::commands::SetNetClasses>,
}

fn nm(value_nm: i64) -> Option<Distance> {
    Some(Distance { value_nm })
}

/// A class as `NETCLASS::Serialize` reports it, with the PCB distances given
/// (clearance, track width) and the schematic widths present or not.
fn class(name: &str, priority: i32, clearance: Option<i64>, track: Option<i64>) -> NetClass {
    NetClass {
        name: name.into(),
        priority: Some(priority),
        board: Some(NetClassBoardSettings {
            clearance: clearance.and_then(nm),
            track_width: track.and_then(nm),
            ..Default::default()
        }),
        schematic: Some(Default::default()),
        r#type: kiapi::common::project::NetClassType::NctExplicit as i32,
        constituents: vec![name.into()],
    }
}

/// KiCad's own Default, as KiCad 10.0 serialises its seeded one.
fn complete_default() -> NetClass {
    let mut default = class("Default", i32::MAX, Some(200_000), Some(200_000));
    let board = default.board.as_mut().unwrap();
    board.diff_pair_track_width = nm(200_000);
    board.diff_pair_gap = nm(250_000);
    board.diff_pair_via_gap = nm(250_000);
    board.via_stack = Some(via_stack(Some(600_000), Some(300_000)));
    default.schematic = Some(kiapi::common::project::NetClassSchematicSettings {
        wire_width: nm(1524),
        bus_width: nm(3048),
        ..Default::default()
    });
    default
}

fn via_stack(diameter: Option<i64>, drill: Option<i64>) -> kiapi::board::types::PadStack {
    let round = |nm| kiapi::common::types::Vector2 { x_nm: nm, y_nm: nm };
    kiapi::board::types::PadStack {
        copper_layers: diameter
            .map(|nm| kiapi::board::types::PadStackLayer {
                size: Some(round(nm)),
                ..Default::default()
            })
            .into_iter()
            .collect(),
        drill: drill.map(|nm| kiapi::board::types::DrillProperties {
            diameter: Some(round(nm)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// `NETCLASS::Deserialize`, for the fields this double keeps.
fn deserialize_into(target: &mut NetClass, message: &NetClass) {
    target.priority = Some(message.priority.unwrap_or(0));
    let incoming = message.board.clone().unwrap_or_default();
    let board = target.board.get_or_insert_with(Default::default);
    for (slot, value) in [
        (&mut board.clearance, incoming.clearance),
        (&mut board.track_width, incoming.track_width),
        (
            &mut board.diff_pair_track_width,
            incoming.diff_pair_track_width,
        ),
        (&mut board.diff_pair_gap, incoming.diff_pair_gap),
        (&mut board.diff_pair_via_gap, incoming.diff_pair_via_gap),
    ] {
        if value.is_some() {
            *slot = value;
        }
    }
    if let Some(stack) = incoming.via_stack {
        let held = board.via_stack.get_or_insert_with(Default::default);
        if !stack.copper_layers.is_empty() {
            held.copper_layers = stack.copper_layers;
        }
        if stack.drill.is_some() {
            held.drill = stack.drill;
        }
    }
}

fn apply_set(state: &mut Kicad, request: &kiapi::common::commands::SetNetClasses) {
    if request.merge_mode == kiapi::common::types::MapMergeMode::MmmReplace as i32 {
        state.classes.retain(|class| class.name == "Default");
    }
    for message in &request.net_classes {
        match state.classes.iter_mut().find(|c| c.name == message.name) {
            Some(existing) => deserialize_into(existing, message),
            None => {
                let mut created = class(&message.name, -1, None, None);
                deserialize_into(&mut created, message);
                state.classes.push(created);
            }
        }
    }
}

fn spawn_kicad(
    document: kiapi::common::types::DocumentSpecifier,
    classes: Vec<NetClass>,
    behaviour: SetBehaviour,
) -> (MockIpcServer, Arc<Mutex<Kicad>>) {
    let state = Arc::new(Mutex::new(Kicad {
        classes,
        ..Default::default()
    }));
    let shared = state.clone();
    let server = MockIpcServer::spawn("netclasses", move |request| {
        let command = request.message.expect("a command");
        let name = command
            .type_url
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_string();
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
            "GetOpenDocuments" => ok(Some(konnect_ipc::builders::pack_any(
                &kiapi::common::commands::GetOpenDocumentsResponse {
                    documents: vec![document.clone()],
                },
                "kiapi.common.commands.GetOpenDocumentsResponse",
            ))),
            "GetNets" => ok(Some(konnect_ipc::builders::pack_any(
                &kiapi::board::commands::NetsResponse {
                    nets: vec![kiapi::board::types::Net {
                        code: Some(kiapi::board::types::NetCode { value: 1 }),
                        name: "LIVE_NET".into(),
                    }],
                },
                "kiapi.board.commands.NetsResponse",
            ))),
            "GetNetClasses" => ok(Some(konnect_ipc::builders::pack_any(
                &kiapi::common::commands::NetClassesResponse {
                    net_classes: state.classes.clone(),
                },
                "kiapi.common.commands.NetClassesResponse",
            ))),
            "SetNetClasses" => {
                let set = kiapi::common::commands::SetNetClasses::decode(command.value.as_slice())
                    .expect("a SetNetClasses request");
                state.sets.push(set.clone());
                match behaviour {
                    SetBehaviour::Apply => {
                        apply_set(&mut state, &set);
                        ok(None)
                    }
                    SetBehaviour::AcceptAndIgnore => ok(None),
                    SetBehaviour::Refuse => kiapi::common::ApiResponse {
                        status: Some(kiapi::common::ApiResponseStatus {
                            status: kiapi::common::ApiStatusCode::AsBadRequest as i32,
                            error_message: "could not unpack netclass 'HV'".into(),
                        }),
                        header: None,
                        message: None,
                    },
                }
            }
            other => panic!("the double was sent {other}"),
        }
    });
    (server, state)
}

/// KiCad holding `project`'s board, with its project loaded and these classes.
fn kicad_holding(
    project: &Project,
    classes: Vec<NetClass>,
    behaviour: SetBehaviour,
) -> (MockIpcServer, Arc<Mutex<Kicad>>) {
    spawn_kicad(
        board_document_in_project(&project.board),
        classes,
        behaviour,
    )
}

// ─── Served calls ────────────────────────────────────────────────────────────

async fn handler_talking_to(address: &str) -> McpHandler {
    McpHandler::new(ServerConfig {
        kicad_cli: String::new(),
        kicad_binary: String::new(),
        ipc_address: address.to_string(),
        project_dir: None,
        jlcpcb_db_path: None,
        auto_load_toolsets: true,
        eager_toolsets: true,
    })
    .await
    .expect("handler builds")
}

async fn call(
    server: &MockIpcServer,
    tool: &str,
    board: &Path,
    arguments: Value,
) -> CallToolResult {
    let handler = handler_talking_to(server.address()).await;
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
    let text = match result.content.first() {
        Some(ToolContent::Text { text }) => text,
        other => panic!("expected text content, got {other:?}"),
    };
    serde_json::from_str(text).unwrap_or_else(|_| json!({ "text": text }))
}

fn kind(result: &CallToolResult) -> Option<String> {
    crate::mcp::error::extract_error_kind(result)
}

fn find<'a>(state: &'a Kicad, name: &str) -> &'a NetClass {
    state
        .classes
        .iter()
        .find(|class| class.name == name)
        .unwrap_or_else(|| panic!("KiCad holds no class {name}"))
}

fn board_nm(
    class: &NetClass,
    pick: fn(&NetClassBoardSettings) -> &Option<Distance>,
) -> Option<i64> {
    class
        .board
        .as_ref()
        .and_then(|board| pick(board).as_ref())
        .map(|distance| distance.value_nm)
}

// ─── create_netclass ─────────────────────────────────────────────────────────

/// The case that used to force closing KiCad: an update while KiCad holds
/// the board. KiCad gets only the named value, the class keeps everything
/// else, and neither file changes.
#[tokio::test]
async fn an_update_goes_to_kicad_and_leaves_both_files_alone() {
    let project = project();
    let before = snapshot(&project);
    let (server, kicad) = kicad_holding(
        &project,
        vec![
            complete_default(),
            class("HV", 2, Some(500_000), Some(300_000)),
        ],
        SetBehaviour::Apply,
    );

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "HV", "trace_width": 0.4 }),
    )
    .await;

    assert!(!result.is_error, "{}", body(&result));
    let answer = body(&result);
    assert_eq!(answer["source"], json!("ipc"), "{answer}");
    assert_eq!(answer["changed"], json!(true), "{answer}");
    assert_eq!(answer["persisted"], json!(false), "{answer}");
    assert_eq!(answer["updated_existing"], json!(true), "{answer}");
    assert_eq!(answer["trace_width"], json!(0.4), "{answer}");
    assert_eq!(answer["clearance"], json!(0.5), "{answer}");
    assert_eq!(answer["priority"], json!(2), "{answer}");
    assert_eq!(
        snapshot(&project),
        before,
        "a file changed under the open editor"
    );

    let state = kicad.lock().unwrap();
    assert_eq!(state.sets.len(), 1);
    let sent = &state.sets[0].net_classes[0];
    assert_eq!(
        sent.priority,
        Some(2),
        "the priority must survive the write"
    );
    assert_eq!(board_nm(sent, |b| &b.track_width), Some(400_000));
    assert_eq!(
        board_nm(sent, |b| &b.clearance),
        None,
        "only the named value is sent"
    );
    let hv = find(&state, "HV");
    assert_eq!(board_nm(hv, |b| &b.track_width), Some(400_000));
    assert_eq!(board_nm(hv, |b| &b.clearance), Some(500_000));
    assert_eq!(hv.priority, Some(2));
}

/// A new class gets the documented defaults and the priority KiCad's own
/// constructor gives one.
#[tokio::test]
async fn a_new_class_is_created_in_kicad_with_the_defaults() {
    let project = project();
    let (server, kicad) = kicad_holding(&project, vec![complete_default()], SetBehaviour::Apply);

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "Power", "clearance": 0.3 }),
    )
    .await;

    assert!(!result.is_error, "{}", body(&result));
    let answer = body(&result);
    assert_eq!(answer["updated_existing"], json!(false), "{answer}");
    assert_eq!(answer["clearance"], json!(0.3), "{answer}");
    assert_eq!(answer["trace_width"], json!(0.25), "{answer}");
    assert_eq!(answer["via_drill"], json!(0.4), "{answer}");
    assert_eq!(answer["via_diameter"], json!(0.8), "{answer}");
    let state = kicad.lock().unwrap();
    let power = find(&state, "Power");
    assert_eq!(power.priority, Some(-1));
    assert_eq!(board_nm(power, |b| &b.clearance), Some(300_000));
}

/// Naming values the class already holds decides nothing, so nothing is sent.
#[tokio::test]
async fn a_call_that_changes_nothing_sends_no_write() {
    let project = project();
    let (server, kicad) = kicad_holding(
        &project,
        vec![complete_default(), class("HV", 2, Some(500_000), None)],
        SetBehaviour::Apply,
    );

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "HV", "clearance": 0.5 }),
    )
    .await;

    assert!(!result.is_error, "{}", body(&result));
    assert_eq!(body(&result)["changed"], json!(false));
    assert!(kicad.lock().unwrap().sets.is_empty());
}

/// The Default is repaired over IPC where the API can say what it writes,
/// and what it cannot repair is reported rather than skipped silently.
#[tokio::test]
async fn an_incomplete_default_is_repaired_where_the_api_allows_and_the_rest_is_named() {
    let project = project();
    let mut holed = class("Default", i32::MAX, Some(200_000), None);
    holed.schematic = Some(Default::default());
    let (server, kicad) = kicad_holding(&project, vec![holed], SetBehaviour::Apply);

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "Default" }),
    )
    .await;

    assert!(!result.is_error, "{}", body(&result));
    let answer = body(&result);
    assert_eq!(answer["complete"], json!(false), "{answer}");
    assert_eq!(
        answer["refused_parts"][0]["fields"],
        json!(["wire_width", "bus_width"]),
        "{answer}"
    );
    let repaired: Vec<&str> = answer["repaired_fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(repaired.contains(&"track_width"), "{answer}");
    assert!(
        !repaired.contains(&"clearance"),
        "a set value is not repaired: {answer}"
    );
    let state = kicad.lock().unwrap();
    let default = find(&state, "Default");
    assert_eq!(board_nm(default, |b| &b.track_width), Some(200_000));
    assert_eq!(board_nm(default, |b| &b.clearance), Some(200_000));
    assert_eq!(default.priority, Some(i32::MAX));
}

/// KiCad answers `AS_OK` and keeps the old value. Only the readback can see
/// that, and it is an error, not a success.
#[tokio::test]
async fn a_write_kicad_did_not_apply_is_a_readback_error() {
    let project = project();
    let before = snapshot(&project);
    let (server, _kicad) = kicad_holding(
        &project,
        vec![complete_default(), class("HV", 2, Some(500_000), None)],
        SetBehaviour::AcceptAndIgnore,
    );

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "HV", "clearance": 0.7 }),
    )
    .await;

    assert!(result.is_error, "{}", body(&result));
    assert_eq!(kind(&result).as_deref(), Some("mutation_outcome_uncertain"));
    assert!(
        body(&result).to_string().contains("clearance"),
        "{}",
        body(&result)
    );
    assert_eq!(snapshot(&project), before);
}

/// KiCad refuses the write: the refusal comes back, and the file is not the
/// fallback for it.
#[tokio::test]
async fn a_refused_write_is_reported_and_not_retried_on_the_file() {
    let project = project();
    let before = snapshot(&project);
    let (server, kicad) = kicad_holding(
        &project,
        vec![complete_default(), class("HV", 2, Some(500_000), None)],
        SetBehaviour::Refuse,
    );

    let result = call(
        &server,
        "create_netclass",
        &project.board,
        json!({ "name": "HV", "clearance": 0.7 }),
    )
    .await;

    assert!(result.is_error, "{}", body(&result));
    assert_eq!(kind(&result).as_deref(), Some("editor_unavailable"));
    assert_eq!(snapshot(&project), before);
    let state = kicad.lock().unwrap();
    assert_eq!(state.sets.len(), 1);
    assert_eq!(
        board_nm(find(&state, "HV"), |b| &b.clearance),
        Some(500_000)
    );
}

/// KiCad holds the board but has another project loaded. Its net class
/// commands would act on that one, so neither is sent.
#[tokio::test]
async fn a_kicad_with_another_project_loaded_is_refused_before_any_net_class_command() {
    let project = project();
    let before = snapshot(&project);
    let elsewhere = tempfile::tempdir().unwrap();
    let mut document = board_document_in_project(&project.board);
    document.project.as_mut().unwrap().path = elsewhere.path().to_string_lossy().into_owned();
    // The board itself is still named by its absolute path, so it binds.
    document.identifier = board_document(&project.board.to_string_lossy()).identifier;
    let (server, kicad) = spawn_kicad(document, vec![complete_default()], SetBehaviour::Apply);

    for (tool, arguments) in [
        ("create_netclass", json!({ "name": "HV", "clearance": 0.7 })),
        ("get_netclasses", json!({})),
    ] {
        let result = call(&server, tool, &project.board, arguments).await;
        assert!(result.is_error, "{tool}: {}", body(&result));
        assert_eq!(kind(&result).as_deref(), Some("wrong_project"), "{tool}");
    }

    assert_eq!(snapshot(&project), before);
    let state = kicad.lock().unwrap();
    assert!(
        !state.commands.iter().any(|c| c.contains("NetClasses")),
        "{:?}",
        state.commands
    );
}

/// The served schema refuses a misspelt or out-of-range argument before the
/// handler runs: nothing reaches KiCad, and no file changes.
#[tokio::test]
async fn a_typo_or_a_negative_distance_is_refused_without_a_write() {
    let project = project();
    let before = snapshot(&project);
    let (server, kicad) = kicad_holding(&project, vec![complete_default()], SetBehaviour::Apply);

    for arguments in [
        json!({ "name": "HV", "trace_widht": 0.4 }),
        json!({ "name": "HV", "clearance": -0.1 }),
    ] {
        let result = call(
            &server,
            "create_netclass",
            &project.board,
            arguments.clone(),
        )
        .await;
        assert!(result.is_error, "{arguments}: {}", body(&result));
        assert_eq!(
            kind(&result).as_deref(),
            Some("invalid_argument"),
            "{arguments}"
        );
    }

    assert_eq!(snapshot(&project), before);
    assert!(kicad.lock().unwrap().commands.is_empty());
}

// ─── get_netclasses and assign_net_to_class ──────────────────────────────────

/// The definitions are KiCad's, not the file's: the fixture's file holds only
/// the Default, and KiCad holds an unsaved HV class.
#[tokio::test]
async fn get_netclasses_reads_the_definitions_kicad_holds() {
    let project = project();
    let (server, _kicad) = kicad_holding(
        &project,
        vec![complete_default(), class("HV", 2, Some(500_000), None)],
        SetBehaviour::Apply,
    );

    let result = call(&server, "get_netclasses", &project.board, json!({})).await;

    assert!(!result.is_error, "{}", body(&result));
    let answer = body(&result);
    assert_eq!(answer["sources"]["definitions"], json!("ipc"), "{answer}");
    assert_eq!(
        answer["sources"]["patterns"],
        json!("project_file"),
        "{answer}"
    );
    assert_eq!(answer["sources"]["board_nets"], json!("ipc"), "{answer}");
    assert!(answer["patterns_note"].is_string(), "{answer}");
    let hv = answer["netclasses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "HV")
        .unwrap_or_else(|| panic!("the live HV class is missing: {answer}"));
    assert_eq!(hv["clearance"], json!(0.5), "{answer}");
    // Not set by HV, so resolved from KiCad's Default.
    assert_eq!(hv["trace_width"], json!(0.2), "{answer}");
    assert_eq!(
        hv["inherits"],
        json!(["trace_width", "via_drill", "via_diameter"])
    );
    let default = &answer["netclasses"][0];
    assert_eq!(default["missing_fields"], json!([]), "{answer}");
    assert_eq!(
        default["fields_not_checked"],
        json!(["microvia_diameter", "microvia_drill", "line_style"]),
        "{answer}"
    );
}

/// Pattern assignment has no IPC command, so a held board is refused with
/// the reason and the way out, and the project file stays as it was.
#[tokio::test]
async fn assign_net_to_class_names_the_missing_capability_while_kicad_holds_the_board() {
    let project = project();
    let before = snapshot(&project);
    let (server, kicad) = kicad_holding(&project, vec![complete_default()], SetBehaviour::Apply);

    let result = call(
        &server,
        "assign_net_to_class",
        &project.board,
        json!({ "net_name": "VCC", "netclass": "Default" }),
    )
    .await;

    assert!(result.is_error, "{}", body(&result));
    assert_eq!(kind(&result).as_deref(), Some("unsupported_capability"));
    assert!(
        body(&result).to_string().contains("Board Setup"),
        "{}",
        body(&result)
    );
    assert_eq!(snapshot(&project), before);
    assert!(kicad.lock().unwrap().sets.is_empty());
}

/// A busy KiCad names no board and holds this one as far as anyone knows.
/// The file path used to let that through; a busy editor is no proof the
/// file is safe to edit.
#[tokio::test]
async fn a_busy_kicad_refuses_the_write_rather_than_letting_the_file_take_it() {
    let project = project();
    let before = snapshot(&project);
    let server = MockIpcServer::spawn("netclasses-busy", |request| {
        assert!(request.message.is_some(), "a command");
        kiapi::common::ApiResponse {
            status: Some(kiapi::common::ApiResponseStatus {
                status: kiapi::common::ApiStatusCode::AsBusy as i32,
                error_message: "KiCad is busy".into(),
            }),
            header: None,
            message: None,
        }
    });

    for (tool, arguments) in [
        ("create_netclass", json!({ "name": "HV", "clearance": 0.7 })),
        (
            "assign_net_to_class",
            json!({ "net_name": "VCC", "netclass": "Default" }),
        ),
    ] {
        let result = call(&server, tool, &project.board, arguments).await;
        assert!(result.is_error, "{tool}: {}", body(&result));
        assert_eq!(
            kind(&result).as_deref(),
            Some("editor_unavailable"),
            "{tool}"
        );
    }
    assert_eq!(snapshot(&project), before);
}
