//! Teardrop settings on pads and vias over KiCad's IPC API.
//!
//! `Pad.teardrop` and `Via.teardrop` exist since KiCad 10.0.7. KiCad builds
//! the teardrop areas itself when a commit that touches a pad, via or
//! footprint is pushed (`BOARD_COMMIT::Push` in `pcbnew/board_commit.cpp`), so
//! writing the settings inside one commit is all a client does; the areas that
//! result are counted from a fresh read rather than assumed.
//!
//! Pads are children of their footprint, and KiCad only updates a footprint as
//! a whole (remove and re-add), so a pad edit sends the complete
//! `FootprintInstance` it was read in, with only the pad's `teardrop` changed.

use crate::builders::{any_is, pack_any};
use crate::client::{footprint_reference, KiCadIpcClient};
use crate::gen::kiapi;
use crate::types::IpcKiCadVersion;
use anyhow::{Context, Result};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const PAD_TYPE: &str = "kiapi.board.types.Pad";
const VIA_TYPE: &str = "kiapi.board.types.Via";
const FOOTPRINT_TYPE: &str = "kiapi.board.types.FootprintInstance";
const ZONE_TYPE: &str = "kiapi.board.types.Zone";

/// The first KiCad release whose pads and vias carry `teardrop`.
pub const TEARDROP_MIN_VERSION: (u32, u32, u32) = (10, 0, 7);

/// Whether this KiCad reads and writes `PadTeardropSettings`.
pub fn kicad_supports_teardrops(version: &IpcKiCadVersion) -> bool {
    (version.major, version.minor, version.patch) >= TEARDROP_MIN_VERSION
}

/// What KiCad's `PadTeardropSettings` holds, distances in nanometres.
///
/// A maximum of 0 means no limit (`TEARDROP_PARAMETERS::m_TdMaxLen`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcTeardropSettings {
    pub enabled: bool,
    pub curved_edges: bool,
    pub allow_multiple_track_segments: bool,
    pub prefer_zone_connection: bool,
    pub max_length_nm: i64,
    pub max_width_nm: i64,
    pub best_length_ratio: f64,
    pub best_width_ratio: f64,
    pub max_track_width_ratio: f64,
}

impl IpcTeardropSettings {
    pub fn from_proto(settings: &kiapi::board::types::PadTeardropSettings) -> Self {
        Self {
            enabled: settings.mode == kiapi::board::types::PadTeardropMode::PtmEnabled as i32,
            curved_edges: settings.curved_edges,
            allow_multiple_track_segments: settings.allow_multiple_track_segments,
            prefer_zone_connection: settings.prefer_zone_connection,
            max_length_nm: settings.max_length.as_ref().map_or(0, |d| d.value_nm),
            max_width_nm: settings.max_width.as_ref().map_or(0, |d| d.value_nm),
            best_length_ratio: settings.best_length_ratio,
            best_width_ratio: settings.best_width_ratio,
            max_track_width_ratio: settings.max_track_width_ratio,
        }
    }
}

/// The settings to change; `None` keeps the item's own value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IpcTeardropPatch {
    pub enabled: Option<bool>,
    pub curved_edges: Option<bool>,
    pub allow_multiple_track_segments: Option<bool>,
    pub prefer_zone_connection: Option<bool>,
    pub max_length_nm: Option<i64>,
    pub max_width_nm: Option<i64>,
    pub best_length_ratio: Option<f64>,
    pub best_width_ratio: Option<f64>,
    pub max_track_width_ratio: Option<f64>,
}

impl IpcTeardropPatch {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Change the named fields of `settings` and leave every other one as read.
    pub fn apply(&self, settings: &mut kiapi::board::types::PadTeardropSettings) {
        use kiapi::board::types::PadTeardropMode;
        use kiapi::common::types::Distance;

        if let Some(enabled) = self.enabled {
            settings.mode = if enabled {
                PadTeardropMode::PtmEnabled
            } else {
                PadTeardropMode::PtmDisabled
            } as i32;
        }
        if let Some(value) = self.curved_edges {
            settings.curved_edges = value;
        }
        if let Some(value) = self.allow_multiple_track_segments {
            settings.allow_multiple_track_segments = value;
        }
        if let Some(value) = self.prefer_zone_connection {
            settings.prefer_zone_connection = value;
        }
        if let Some(value_nm) = self.max_length_nm {
            settings.max_length = Some(Distance { value_nm });
        }
        if let Some(value_nm) = self.max_width_nm {
            settings.max_width = Some(Distance { value_nm });
        }
        if let Some(value) = self.best_length_ratio {
            settings.best_length_ratio = value;
        }
        if let Some(value) = self.best_width_ratio {
            settings.best_width_ratio = value;
        }
        if let Some(value) = self.max_track_width_ratio {
            settings.max_track_width_ratio = value;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcTeardropItemKind {
    Pad,
    Via,
}

/// One pad or via and its teardrop settings, as KiCad reported them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcTeardropItem {
    pub kind: IpcTeardropItemKind,
    pub id: String,
    pub net: String,
    /// The parent footprint's reference designator; pads only.
    pub footprint_reference: Option<String>,
    /// The parent footprint's KIID; pads only.
    pub footprint_id: Option<String>,
    /// Pads only.
    pub pad_number: Option<String>,
    /// `pth`, `smd`, `edge_connector`, `npth` or `unknown`; pads only.
    pub pad_type: Option<String>,
    /// `None` when the message carried no `teardrop` (KiCad before 10.0.7).
    pub settings: Option<IpcTeardropSettings>,
}

/// Teardrop areas on copper layers, by what KiCad built them for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcTeardropAreaCount {
    pub pad_or_via: usize,
    pub track_end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcTeardropSnapshot {
    pub items: Vec<IpcTeardropItem>,
    pub areas: IpcTeardropAreaCount,
}

/// Which pads and vias an operation addresses. Every non-empty filter must
/// match; an empty selection addresses every pad and via.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IpcTeardropSelection {
    pub item_ids: Vec<String>,
    pub nets: Vec<String>,
    /// Pads of these footprints; excludes vias.
    pub references: Vec<String>,
    /// Empty means pads and vias.
    pub kinds: Vec<IpcTeardropItemKind>,
    /// Pads of these types (`pth`, `smd`, `edge_connector`, `npth`); excludes vias.
    pub pad_types: Vec<String>,
}

/// A selection naming something the board does not have, or matching nothing.
/// Raised before anything is written.
#[derive(Debug, Clone, PartialEq)]
pub enum TeardropSelectionError {
    UnknownItemIds(Vec<String>),
    UnknownReferences(Vec<String>),
    UnknownNets(Vec<String>),
    NothingSelected,
    /// Selected items whose message carried no `teardrop` field.
    SettingsUnavailable(Vec<String>),
}

impl std::fmt::Display for TeardropSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownItemIds(ids) => {
                write!(f, "no pad or via has the id {}", ids.join(", "))
            }
            Self::UnknownReferences(refs) => {
                write!(
                    f,
                    "no footprint with pads has the reference {}",
                    refs.join(", ")
                )
            }
            Self::UnknownNets(nets) => {
                write!(f, "no pad or via is on the net {}", nets.join(", "))
            }
            Self::NothingSelected => write!(f, "the filters together match no pad or via"),
            Self::SettingsUnavailable(ids) => write!(
                f,
                "KiCad reported no teardrop settings for {}",
                ids.join(", ")
            ),
        }
    }
}

impl std::error::Error for TeardropSelectionError {}

impl IpcTeardropSelection {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Indices of the selected items, or why the selection is refused.
    pub fn select(&self, items: &[IpcTeardropItem]) -> Result<Vec<usize>, TeardropSelectionError> {
        let unknown = |wanted: &[String], have: &dyn Fn(&IpcTeardropItem) -> Option<&str>| {
            let present: BTreeSet<&str> = items.iter().filter_map(have).collect();
            let mut missing: Vec<String> = wanted
                .iter()
                .filter(|w| !present.contains(w.as_str()))
                .cloned()
                .collect();
            missing.dedup();
            missing
        };
        let missing = unknown(&self.item_ids, &|i| Some(i.id.as_str()));
        if !missing.is_empty() {
            return Err(TeardropSelectionError::UnknownItemIds(missing));
        }
        let missing = unknown(&self.references, &|i| i.footprint_reference.as_deref());
        if !missing.is_empty() {
            return Err(TeardropSelectionError::UnknownReferences(missing));
        }
        let missing = unknown(&self.nets, &|i| Some(i.net.as_str()));
        if !missing.is_empty() {
            return Err(TeardropSelectionError::UnknownNets(missing));
        }

        let selected: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.matches(item))
            .map(|(index, _)| index)
            .collect();
        if selected.is_empty() {
            return Err(TeardropSelectionError::NothingSelected);
        }
        Ok(selected)
    }

    fn matches(&self, item: &IpcTeardropItem) -> bool {
        let within = |filter: &[String], value: Option<&str>| {
            filter.is_empty() || value.is_some_and(|v| filter.iter().any(|f| f == v))
        };
        (self.kinds.is_empty() || self.kinds.contains(&item.kind))
            && within(&self.item_ids, Some(&item.id))
            && within(&self.nets, Some(&item.net))
            && within(&self.references, item.footprint_reference.as_deref())
            && within(&self.pad_types, item.pad_type.as_deref())
    }
}

/// One item whose settings after the write are not the ones expected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcTeardropMismatch {
    pub id: String,
    pub expected: Option<IpcTeardropSettings>,
    /// `None` when the item is gone or carried no settings.
    pub observed: Option<IpcTeardropSettings>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IpcTeardropEditOutcome {
    /// A fresh read shows every selected item with the requested settings and
    /// every other pad and via as it was.
    Applied {
        selected: Vec<String>,
        changed: Vec<String>,
        before: IpcTeardropSnapshot,
        after: IpcTeardropSnapshot,
    },
    /// KiCad may have changed the board, and the fresh read did not prove the
    /// requested state.
    Uncertain {
        selected: Vec<String>,
        changed: Vec<String>,
        mismatches: Vec<IpcTeardropMismatch>,
        /// KiCad answered the update without an error.
        accepted: bool,
        reason: String,
    },
}

fn pad_type_name(value: i32) -> &'static str {
    use kiapi::board::types::PadType;
    match PadType::try_from(value) {
        Ok(PadType::PtPth) => "pth",
        Ok(PadType::PtSmd) => "smd",
        Ok(PadType::PtEdgeConnector) => "edge_connector",
        Ok(PadType::PtNpth) => "npth",
        _ => "unknown",
    }
}

fn net_name(net: Option<&kiapi::board::types::Net>) -> String {
    net.map(|n| n.name.clone()).unwrap_or_default()
}

fn kiid(id: Option<&kiapi::common::types::Kiid>) -> String {
    id.map(|k| k.value.clone()).unwrap_or_default()
}

fn is_copper_teardrop(
    zone: &kiapi::board::types::Zone,
) -> Option<kiapi::board::types::ZoneTeardropType> {
    use kiapi::board::types::{zone::Settings, BoardLayer, ZoneTeardropType, ZoneType};
    if zone.r#type != ZoneType::ZtTeardrop as i32 {
        return None;
    }
    // KiCad also builds a solder-mask twin for a teardrop on an outer layer.
    let on_copper = zone
        .layers
        .iter()
        .any(|layer| BoardLayer::try_from(*layer).is_ok_and(|l| l.as_str_name().ends_with("_Cu")));
    if !on_copper {
        return None;
    }
    let kind = match &zone.settings {
        Some(Settings::CopperSettings(copper)) => copper
            .teardrop
            .as_ref()
            .and_then(|t| ZoneTeardropType::try_from(t.r#type).ok())
            .unwrap_or(ZoneTeardropType::ZtdtUnknown),
        _ => ZoneTeardropType::ZtdtUnknown,
    };
    Some(kind)
}

/// The board as read for teardrop work: decoded items, and the raw
/// footprints and vias they came from, for a write that changes nothing else.
struct TeardropBoard {
    footprints: Vec<kiapi::board::types::FootprintInstance>,
    vias: Vec<kiapi::board::types::Via>,
    /// Each pad's and via's `teardrop` exactly as sent, by KIID.
    raw_settings: BTreeMap<String, kiapi::board::types::PadTeardropSettings>,
    snapshot: IpcTeardropSnapshot,
}

fn read_board(items: &[prost_types::Any]) -> Result<TeardropBoard> {
    let mut footprints = Vec::new();
    let mut vias = Vec::new();
    let mut entries = Vec::new();
    let mut areas = IpcTeardropAreaCount::default();
    let mut raw_settings = BTreeMap::new();

    for item in items {
        if any_is(item, FOOTPRINT_TYPE) {
            let footprint = kiapi::board::types::FootprintInstance::decode(item.value.as_slice())
                .context("KiCad sent a footprint that does not decode")?;
            let reference = footprint_reference(&footprint).to_string();
            let footprint_id = kiid(footprint.id.as_ref());
            for child in footprint.definition.iter().flat_map(|d| d.items.iter()) {
                if !any_is(child, PAD_TYPE) {
                    continue;
                }
                let pad = kiapi::board::types::Pad::decode(child.value.as_slice())
                    .with_context(|| format!("a pad of {reference} does not decode"))?;
                if let Some(settings) = &pad.teardrop {
                    raw_settings.insert(kiid(pad.id.as_ref()), *settings);
                }
                entries.push(IpcTeardropItem {
                    kind: IpcTeardropItemKind::Pad,
                    id: kiid(pad.id.as_ref()),
                    net: net_name(pad.net.as_ref()),
                    footprint_reference: Some(reference.clone()),
                    footprint_id: Some(footprint_id.clone()),
                    pad_number: Some(pad.number.clone()),
                    pad_type: Some(pad_type_name(pad.r#type).to_string()),
                    settings: pad.teardrop.as_ref().map(IpcTeardropSettings::from_proto),
                });
            }
            footprints.push(footprint);
        } else if any_is(item, VIA_TYPE) {
            let via = kiapi::board::types::Via::decode(item.value.as_slice())
                .context("KiCad sent a via that does not decode")?;
            if let Some(settings) = &via.teardrop {
                raw_settings.insert(kiid(via.id.as_ref()), *settings);
            }
            entries.push(IpcTeardropItem {
                kind: IpcTeardropItemKind::Via,
                id: kiid(via.id.as_ref()),
                net: net_name(via.net.as_ref()),
                footprint_reference: None,
                footprint_id: None,
                pad_number: None,
                pad_type: None,
                settings: via.teardrop.as_ref().map(IpcTeardropSettings::from_proto),
            });
            vias.push(via);
        } else if any_is(item, ZONE_TYPE) {
            let zone = kiapi::board::types::Zone::decode(item.value.as_slice())
                .context("KiCad sent a zone that does not decode")?;
            match is_copper_teardrop(&zone) {
                Some(kiapi::board::types::ZoneTeardropType::ZtdtTrackEnd) => areas.track_end += 1,
                Some(_) => areas.pad_or_via += 1,
                None => {}
            }
        }
    }

    Ok(TeardropBoard {
        footprints,
        vias,
        raw_settings,
        snapshot: IpcTeardropSnapshot {
            items: entries,
            areas,
        },
    })
}

/// The `UpdateItems` payload that gives every item in `changes` its patched
/// settings: one `Via` per changed via, one complete `FootprintInstance` per
/// footprint with a changed pad. Nothing else in a message differs from what
/// was read.
fn build_updates(
    board: &TeardropBoard,
    changes: &BTreeSet<String>,
    patch: &IpcTeardropPatch,
) -> Result<Vec<prost_types::Any>> {
    let mut updates = Vec::new();
    for footprint in &board.footprints {
        let mut edited = footprint.clone();
        let mut touched = false;
        if let Some(definition) = edited.definition.as_mut() {
            for child in definition.items.iter_mut() {
                if !any_is(child, PAD_TYPE) {
                    continue;
                }
                let mut pad = kiapi::board::types::Pad::decode(child.value.as_slice())?;
                if !changes.contains(&kiid(pad.id.as_ref())) {
                    continue;
                }
                patch.apply(pad.teardrop.get_or_insert_with(Default::default));
                child.value = pad.encode_to_vec();
                touched = true;
            }
        }
        if touched {
            updates.push(pack_any(&edited, FOOTPRINT_TYPE));
        }
    }
    for via in &board.vias {
        if changes.contains(&kiid(via.id.as_ref())) {
            let mut edited = via.clone();
            patch.apply(edited.teardrop.get_or_insert_with(Default::default));
            updates.push(pack_any(&edited, VIA_TYPE));
        }
    }
    Ok(updates)
}

fn mismatches(
    expected: &BTreeMap<String, Option<IpcTeardropSettings>>,
    after: &IpcTeardropSnapshot,
) -> Vec<IpcTeardropMismatch> {
    let observed: BTreeMap<&str, &Option<IpcTeardropSettings>> = after
        .items
        .iter()
        .map(|item| (item.id.as_str(), &item.settings))
        .collect();
    expected
        .iter()
        .filter_map(|(id, want)| {
            let got = observed.get(id.as_str()).and_then(|s| (*s).clone());
            (got != *want).then(|| IpcTeardropMismatch {
                id: id.clone(),
                expected: want.clone(),
                observed: got,
            })
        })
        .collect()
}

impl KiCadIpcClient {
    /// Every pad and via with its teardrop settings, and the teardrop areas
    /// on copper, from one `GetItems`.
    pub fn teardrop_snapshot_in(
        &self,
        document: kiapi::common::types::DocumentSpecifier,
    ) -> Result<IpcTeardropSnapshot> {
        Ok(self.read_teardrop_board(document)?.snapshot)
    }

    fn read_teardrop_board(
        &self,
        document: kiapi::common::types::DocumentSpecifier,
    ) -> Result<TeardropBoard> {
        use kiapi::common::types::KiCadObjectType;
        let items = self.get_items_of_types_in(
            document,
            &[
                KiCadObjectType::KotPcbFootprint,
                KiCadObjectType::KotPcbVia,
                KiCadObjectType::KotPcbZone,
            ],
        )?;
        read_board(&items)
    }

    /// Give the selected pads and vias the patched settings in one KiCad
    /// commit, then prove the result with a fresh read. A selection that
    /// names something missing is refused with a [`TeardropSelectionError`]
    /// before anything is sent. This method never saves the board.
    pub fn set_teardrops_in(
        &self,
        document: kiapi::common::types::DocumentSpecifier,
        selection: &IpcTeardropSelection,
        patch: &IpcTeardropPatch,
    ) -> Result<IpcTeardropEditOutcome> {
        let board = self.read_teardrop_board(document.clone())?;
        let before = board.snapshot.clone();
        let indices = selection.select(&before.items)?;

        let unavailable: Vec<String> = indices
            .iter()
            .map(|&i| &before.items[i])
            .filter(|item| item.settings.is_none())
            .map(|item| item.id.clone())
            .collect();
        if !unavailable.is_empty() {
            return Err(TeardropSelectionError::SettingsUnavailable(unavailable).into());
        }

        let mut expected: BTreeMap<String, Option<IpcTeardropSettings>> = before
            .items
            .iter()
            .map(|item| (item.id.clone(), item.settings.clone()))
            .collect();
        let mut selected = Vec::new();
        let mut changed = BTreeSet::new();
        for &index in &indices {
            let item = &before.items[index];
            let current = item.settings.clone().expect("checked above");
            let mut proto = board.raw_settings[&item.id];
            patch.apply(&mut proto);
            let wanted = IpcTeardropSettings::from_proto(&proto);
            selected.push(item.id.clone());
            if wanted != current {
                changed.insert(item.id.clone());
                expected.insert(item.id.clone(), Some(wanted));
            }
        }
        let changed_ids: Vec<String> = changed.iter().cloned().collect();

        if changed.is_empty() {
            return Ok(IpcTeardropEditOutcome::Applied {
                selected,
                changed: changed_ids,
                after: before.clone(),
                before,
            });
        }

        let updates = build_updates(&board, &changed, patch)?;
        let mutation = self.run_commit("Set teardrops", |client| {
            client.update_items_in(document.clone(), updates)
        });
        let readback = self.teardrop_snapshot_in(document);

        let unchanged: BTreeMap<String, Option<IpcTeardropSettings>> = before
            .items
            .iter()
            .map(|item| (item.id.clone(), item.settings.clone()))
            .collect();
        match (mutation, readback) {
            (mutation, Ok(after)) => {
                let wrong = mismatches(&expected, &after);
                if wrong.is_empty() {
                    return Ok(IpcTeardropEditOutcome::Applied {
                        selected,
                        changed: changed_ids,
                        before,
                        after,
                    });
                }
                match mutation {
                    Err(error) if mismatches(&unchanged, &after).is_empty() => Err(error).context(
                        "KiCad rejected the teardrop edit; a fresh read confirmed no pad or via changed",
                    ),
                    Ok(()) => Ok(IpcTeardropEditOutcome::Uncertain {
                        selected,
                        changed: changed_ids,
                        mismatches: wrong,
                        accepted: true,
                        reason: "KiCad accepted the update, but a fresh read does not show the requested teardrop settings".into(),
                    }),
                    Err(error) => Ok(IpcTeardropEditOutcome::Uncertain {
                        selected,
                        changed: changed_ids,
                        mismatches: wrong,
                        accepted: false,
                        reason: format!(
                            "the update returned an error and a fresh read shows neither the old nor the requested settings: {error:#}"
                        ),
                    }),
                }
            }
            (mutation, Err(read_error)) => Ok(IpcTeardropEditOutcome::Uncertain {
                selected,
                changed: changed_ids,
                mismatches: Vec::new(),
                accepted: mutation.is_ok(),
                reason: match mutation {
                    Ok(()) => format!("KiCad accepted the update, but the board could not be read back: {read_error:#}"),
                    Err(error) => format!("the update returned an error ({error:#}) and the board could not be read back: {read_error:#}"),
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiapi::board::types::{PadTeardropMode, PadTeardropSettings};
    use kiapi::common::types::Distance;

    fn settings() -> PadTeardropSettings {
        PadTeardropSettings {
            mode: PadTeardropMode::PtmDisabled as i32,
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

    fn version(major: u32, minor: u32, patch: u32) -> IpcKiCadVersion {
        IpcKiCadVersion {
            major,
            minor,
            patch,
            full_version: format!("{major}.{minor}.{patch}"),
        }
    }

    #[test]
    fn teardrops_need_kicad_10_0_7() {
        assert!(!kicad_supports_teardrops(&version(10, 0, 6)));
        assert!(!kicad_supports_teardrops(&version(9, 99, 99)));
        assert!(kicad_supports_teardrops(&version(10, 0, 7)));
        assert!(kicad_supports_teardrops(&version(10, 0, 8)));
        assert!(kicad_supports_teardrops(&version(10, 99, 0)));
        assert!(kicad_supports_teardrops(&version(11, 0, 0)));
    }

    #[test]
    fn patch_changes_only_named_fields() {
        let mut target = settings();
        IpcTeardropPatch {
            enabled: Some(true),
            max_width_nm: Some(0),
            ..Default::default()
        }
        .apply(&mut target);
        let mut expected = settings();
        expected.mode = PadTeardropMode::PtmEnabled as i32;
        expected.max_width = Some(Distance { value_nm: 0 });
        assert_eq!(target, expected);

        let mut untouched = settings();
        IpcTeardropPatch::default().apply(&mut untouched);
        assert_eq!(untouched, settings());
    }

    #[test]
    fn absent_maximum_reads_as_no_limit() {
        let mut proto = settings();
        proto.max_length = None;
        assert_eq!(IpcTeardropSettings::from_proto(&proto).max_length_nm, 0);
    }

    fn item(
        kind: IpcTeardropItemKind,
        id: &str,
        net: &str,
        reference: Option<&str>,
    ) -> IpcTeardropItem {
        IpcTeardropItem {
            kind,
            id: id.into(),
            net: net.into(),
            footprint_reference: reference.map(Into::into),
            footprint_id: reference.map(|r| format!("fp-{r}")),
            pad_number: reference.map(|_| "1".into()),
            pad_type: reference.map(|_| "smd".into()),
            settings: Some(IpcTeardropSettings::from_proto(&settings())),
        }
    }

    fn board() -> Vec<IpcTeardropItem> {
        vec![
            item(IpcTeardropItemKind::Pad, "p1", "GND", Some("R1")),
            item(IpcTeardropItemKind::Pad, "p2", "VCC", Some("R1")),
            item(IpcTeardropItemKind::Via, "v1", "GND", None),
        ]
    }

    #[test]
    fn selection_filters_combine() {
        let items = board();
        let all = IpcTeardropSelection::default().select(&items).unwrap();
        assert_eq!(all, vec![0, 1, 2]);

        let gnd = IpcTeardropSelection {
            nets: vec!["GND".into()],
            ..Default::default()
        };
        assert_eq!(gnd.select(&items).unwrap(), vec![0, 2]);

        let gnd_vias = IpcTeardropSelection {
            nets: vec!["GND".into()],
            kinds: vec![IpcTeardropItemKind::Via],
            ..Default::default()
        };
        assert_eq!(gnd_vias.select(&items).unwrap(), vec![2]);

        // A footprint filter leaves vias out.
        let r1 = IpcTeardropSelection {
            references: vec!["R1".into()],
            ..Default::default()
        };
        assert_eq!(r1.select(&items).unwrap(), vec![0, 1]);
    }

    #[test]
    fn selection_names_what_the_board_lacks() {
        let items = board();
        let refuse = |selection: IpcTeardropSelection| selection.select(&items).unwrap_err();
        assert_eq!(
            refuse(IpcTeardropSelection {
                item_ids: vec!["p1".into(), "gone".into()],
                ..Default::default()
            }),
            TeardropSelectionError::UnknownItemIds(vec!["gone".into()])
        );
        assert_eq!(
            refuse(IpcTeardropSelection {
                references: vec!["U9".into()],
                ..Default::default()
            }),
            TeardropSelectionError::UnknownReferences(vec!["U9".into()])
        );
        assert_eq!(
            refuse(IpcTeardropSelection {
                nets: vec!["GDN".into()],
                ..Default::default()
            }),
            TeardropSelectionError::UnknownNets(vec!["GDN".into()])
        );
        assert_eq!(
            refuse(IpcTeardropSelection {
                nets: vec!["VCC".into()],
                kinds: vec![IpcTeardropItemKind::Via],
                ..Default::default()
            }),
            TeardropSelectionError::NothingSelected
        );
    }

    fn pad(id: &str) -> kiapi::board::types::Pad {
        kiapi::board::types::Pad {
            id: Some(kiapi::common::types::Kiid { value: id.into() }),
            number: "1".into(),
            net: Some(kiapi::board::types::Net {
                code: Some(kiapi::board::types::NetCode { value: 3 }),
                name: "GND".into(),
            }),
            r#type: kiapi::board::types::PadType::PtSmd as i32,
            position: Some(kiapi::common::types::Vector2 { x_nm: 10, y_nm: 20 }),
            pad_to_die_length: Some(Distance { value_nm: 7 }),
            teardrop: Some(settings()),
            ..Default::default()
        }
    }

    #[test]
    fn a_pad_update_resends_its_footprint_with_only_that_teardrop_changed() {
        let graphic = prost_types::Any {
            type_url: "type.googleapis.com/kiapi.board.types.BoardGraphicShape".into(),
            value: vec![0x0a, 0x02, 0x08, 0x01],
        };
        let footprint = kiapi::board::types::FootprintInstance {
            id: Some(kiapi::common::types::Kiid { value: "fp".into() }),
            definition: Some(kiapi::board::types::Footprint {
                items: vec![
                    pack_any(&pad("p1"), PAD_TYPE),
                    graphic.clone(),
                    pack_any(&pad("p2"), PAD_TYPE),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let untouched = kiapi::board::types::FootprintInstance {
            id: Some(kiapi::common::types::Kiid {
                value: "other".into(),
            }),
            ..Default::default()
        };
        let items = vec![
            pack_any(&footprint, FOOTPRINT_TYPE),
            pack_any(&untouched, FOOTPRINT_TYPE),
        ];
        let board = read_board(&items).unwrap();
        let patch = IpcTeardropPatch {
            enabled: Some(true),
            ..Default::default()
        };
        let updates = build_updates(&board, &BTreeSet::from(["p1".to_string()]), &patch).unwrap();

        assert_eq!(updates.len(), 1, "only the footprint holding p1 is sent");
        let sent =
            kiapi::board::types::FootprintInstance::decode(updates[0].value.as_slice()).unwrap();
        let sent_items = &sent.definition.as_ref().unwrap().items;
        let mut p1 = kiapi::board::types::Pad::decode(sent_items[0].value.as_slice()).unwrap();
        assert_eq!(
            p1.teardrop.as_ref().unwrap().mode,
            PadTeardropMode::PtmEnabled as i32
        );
        p1.teardrop = Some(settings());
        assert_eq!(p1, pad("p1"), "no other pad field changed");
        assert_eq!(
            sent_items[1], graphic,
            "other children are resent byte for byte"
        );
        assert_eq!(
            sent_items[2],
            footprint.definition.as_ref().unwrap().items[2]
        );
        let mut rest = sent.clone();
        rest.definition = footprint.definition.clone();
        assert_eq!(rest, footprint, "no footprint field changed");
    }

    #[test]
    fn only_copper_teardrop_zones_are_counted() {
        use kiapi::board::types::{
            zone::Settings, BoardLayer, CopperZoneSettings, Zone, ZoneTeardropSettings,
            ZoneTeardropType, ZoneType,
        };
        let zone = |kind: ZoneType, layer: BoardLayer, td: ZoneTeardropType| Zone {
            r#type: kind as i32,
            layers: vec![layer as i32],
            settings: Some(Settings::CopperSettings(CopperZoneSettings {
                teardrop: Some(ZoneTeardropSettings { r#type: td as i32 }),
                ..Default::default()
            })),
            ..Default::default()
        };
        let items: Vec<_> = [
            zone(
                ZoneType::ZtTeardrop,
                BoardLayer::BlFCu,
                ZoneTeardropType::ZtdtViaPad,
            ),
            zone(
                ZoneType::ZtTeardrop,
                BoardLayer::BlFMask,
                ZoneTeardropType::ZtdtViaPad,
            ),
            zone(
                ZoneType::ZtTeardrop,
                BoardLayer::BlIn1Cu,
                ZoneTeardropType::ZtdtTrackEnd,
            ),
            zone(
                ZoneType::ZtCopper,
                BoardLayer::BlBCu,
                ZoneTeardropType::ZtdtNone,
            ),
        ]
        .iter()
        .map(|z| pack_any(z, ZONE_TYPE))
        .collect();
        assert_eq!(
            read_board(&items).unwrap().snapshot.areas,
            IpcTeardropAreaCount {
                pad_or_via: 1,
                track_end: 1
            }
        );
    }
}
