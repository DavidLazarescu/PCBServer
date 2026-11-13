//! Items KiCad sends must survive decode and re-encode unchanged.
//!
//! prost drops fields its definitions do not declare, so an item read from a
//! newer KiCad, modified and sent back through `UpdateItems` silently loses
//! them, and KiCad then applies the absent field's meaning (for a via:
//! teardrops off). Each test hand-encodes a message with the fields KiCad
//! 10.0.7 and 10.0.8 added, using the field numbers from
//! `proto/board/board_types.proto` and `proto/common/types/base_types.proto`,
//! and requires the re-encoded bytes to be identical. The expected bytes are
//! built without the generated types (enum values are written as numbers
//! for the same reason), so a definition that lacks a field fails here
//! instead of agreeing with itself.

use konnect_ipc::gen::kiapi;
use prost::Message;

// ─── Protobuf wire encoding, written out by hand ─────────────────────────────

fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return out;
        }
        out.push(byte | 0x80);
    }
}

fn key(field: u32, wire_type: u8) -> Vec<u8> {
    varint((u64::from(field) << 3) | u64::from(wire_type))
}

/// A varint field: integers, enums and `true` booleans.
fn uint(field: u32, value: u64) -> Vec<u8> {
    [key(field, 0), varint(value)].concat()
}

fn double(field: u32, value: f64) -> Vec<u8> {
    [key(field, 1), value.to_le_bytes().to_vec()].concat()
}

fn bytes(field: u32, value: &[u8]) -> Vec<u8> {
    [key(field, 2), varint(value.len() as u64), value.to_vec()].concat()
}

fn string(field: u32, value: &str) -> Vec<u8> {
    bytes(field, value.as_bytes())
}

fn message(field: u32, parts: &[Vec<u8>]) -> Vec<u8> {
    bytes(field, &parts.concat())
}

/// `kiapi.common.types.KIID` and `Distance` both carry one field 1.
fn kiid(field: u32, value: &str) -> Vec<u8> {
    message(field, &[string(1, value)])
}

fn distance(field: u32, nm: u64) -> Vec<u8> {
    message(field, &[uint(1, nm)])
}

/// Decode `wire` as `M`, re-encode it, and require the same bytes.
fn round_trip<M: Message + Default>(wire: &[u8]) -> M {
    let decoded = M::decode(wire).expect("the hand-encoded message must decode");
    assert_eq!(
        decoded.encode_to_vec(),
        wire,
        "{} lost or changed a field on re-encode",
        std::any::type_name::<M>()
    );
    decoded
}

// ─── Shared sub-messages ─────────────────────────────────────────────────────

/// `PadTeardropSettings` (Since 10.0.7) with every field away from its default.
fn pad_teardrop(field: u32) -> Vec<u8> {
    message(
        field,
        &[
            uint(1, 2),             // mode: PTM_ENABLED
            uint(2, 1),             // curved_edges
            uint(3, 1),             // allow_multiple_track_segments
            uint(4, 1),             // prefer_zone_connection
            distance(5, 1_000_000), // max_length
            distance(6, 2_000_000), // max_width
            double(7, 0.5),         // best_length_ratio
            double(8, 1.0),         // best_width_ratio
            double(9, 0.9),         // max_track_width_ratio
        ],
    )
}

/// Two `ZoneLayerOverrideEntry` items (Since 10.0.7).
fn zone_layer_overrides(field: u32) -> Vec<u8> {
    use kiapi::board::types::BoardLayer;
    [
        message(
            field,
            &[
                uint(1, BoardLayer::BlFCu as u64),
                uint(2, 2), // ZLO_FORCE_FLASHED
            ],
        ),
        message(
            field,
            &[
                uint(1, BoardLayer::BlBCu as u64),
                uint(2, 3), // ZLO_FORCE_NO_ZONE_CONNECTION
            ],
        ),
    ]
    .concat()
}

/// `SolderMaskOverrides` with a margin and `expose_copper` (Since 10.0.7).
fn solder_mask(field: u32) -> Vec<u8> {
    message(field, &[distance(1, 50_000), uint(2, 1)])
}

// ─── Items ───────────────────────────────────────────────────────────────────

#[test]
fn via_keeps_teardrops_zone_overrides_and_free_flag() {
    let wire = [
        kiid(1, "via-kiid"),
        pad_teardrop(8),
        zone_layer_overrides(9),
        uint(10, 1), // is_free
    ]
    .concat();
    round_trip::<kiapi::board::types::Via>(&wire);
}

#[test]
fn pad_keeps_teardrops_zone_overrides_and_fabrication_property() {
    let wire = [
        kiid(1, "pad-kiid"),
        string(3, "1"),
        pad_teardrop(14),
        zone_layer_overrides(15),
        uint(16, 5), // fab_property: PFP_TESTPOINT
    ]
    .concat();
    round_trip::<kiapi::board::types::Pad>(&wire);
}

#[test]
fn copper_zone_keeps_corner_smoothing() {
    let copper_settings = message(
        6,
        &[
            message(9, &[uint(1, 3)]), // teardrop: ZTDT_VIA_PAD
            uint(11, 3),               // corner_smoothing: ZCSM_FILLET
            distance(12, 250_000),     // corner_radius
        ],
    );
    let wire = [kiid(1, "zone-kiid"), copper_settings].concat();
    round_trip::<kiapi::board::types::Zone>(&wire);
}

#[test]
fn footprint_instance_keeps_embedded_files_variants_and_symbol_units() {
    let embedded_files = message(
        18,
        &[message(
            1,
            &[
                string(1, "part.step"),
                uint(2, 3), // EFT_MODEL
                bytes(3, b"compressed-and-encoded"),
                string(4, "0123456789abcdef"),
            ],
        )],
    );
    // One map entry only: prost holds `map<>` fields in a HashMap, whose
    // iteration order is not the order the entries arrived in.
    let variant = message(
        20,
        &[
            string(1, "Lite"),
            uint(2, 1), // do_not_populate
            uint(3, 1), // exclude_from_bill_of_materials
            uint(4, 1), // exclude_from_position_files
            message(6, &[string(1, "Value"), string(2, "10k")]),
        ],
    );
    let unit = message(21, &[string(1, "A"), string(2, "1"), string(2, "2")]);
    let wire = [kiid(1, "fp-kiid"), embedded_files, variant, unit].concat();
    round_trip::<kiapi::board::types::FootprintInstance>(&wire);
}

#[test]
fn track_arc_and_graphic_shape_keep_exposed_copper() {
    round_trip::<kiapi::board::types::Track>(&[kiid(1, "track-kiid"), solder_mask(9)].concat());
    round_trip::<kiapi::board::types::Arc>(&[kiid(1, "arc-kiid"), solder_mask(10)].concat());
    round_trip::<kiapi::board::types::BoardGraphicShape>(
        &[kiid(4, "shape-kiid"), solder_mask(7)].concat(),
    );
}

#[test]
fn text_box_keeps_border_margins_and_knockout() {
    let textbox = message(
        2,
        &[
            string(6, "note"),
            uint(8, 1), // border_enabled
            distance(9, 100_000),
            distance(10, 200_000),
            distance(11, 300_000),
            distance(12, 400_000),
        ],
    );
    let wire = [kiid(1, "textbox-kiid"), textbox, uint(6, 1)].concat();
    round_trip::<kiapi::board::types::BoardTextBox>(&wire);
}

#[test]
fn group_keeps_lock_and_design_block_link() {
    let wire = [
        kiid(1, "group-kiid"),
        string(2, "block"),
        uint(5, kiapi::common::types::LockedState::LsLocked as u64),
        message(6, &[string(1, "Blocks"), string(2, "regulator")]),
    ]
    .concat();
    round_trip::<kiapi::board::types::Group>(&wire);
}

#[test]
fn commit_boundaries_carry_their_document_header() {
    let header = message(
        1,
        &[message(
            1,
            &[uint(
                1,
                kiapi::common::types::DocumentType::DoctypePcb as u64,
            )],
        )],
    );
    round_trip::<kiapi::common::commands::BeginCommit>(&header);

    let header = message(
        4,
        &[message(
            1,
            &[uint(
                1,
                kiapi::common::types::DocumentType::DoctypePcb as u64,
            )],
        )],
    );
    round_trip::<kiapi::common::commands::EndCommit>(&[kiid(1, "commit"), header].concat());
}
