use super::*;
use crate::hlc::HlcTimestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const RELOCATION_DIR: &str = "_relocations";
const RELOCATION_TOMBSTONES_FILE: &str = "_relocation_tombstones.bin";
const RECEIPT_LIMIT: usize = 4_096;
const RELOCATION_MOVE_ID_META: &str = "relocation_move_id";
const RELOCATION_REQUEST_HASH_META: &str = "relocation_request_hash";

pub(super) type ReceiptIndex = BTreeMap<(HlcTimestamp, [u8; 16]), ()>;
pub(super) type RelocationTombstones = BTreeMap<[u8; 16], [u8; 32]>;
pub(super) type ActiveRelocations = BTreeMap<[u8; 16], RelocationReservation>;

#[derive(Clone, Debug)]
pub(super) struct RelocationReservation {
    source_note_id: [u8; 16],
    destination_note_id: [u8; 16],
    block_bids: Vec<[u8; 16]>,
}

impl RelocationReservation {
    fn from_intent(intent: &RelocationIntent) -> Self {
        Self {
            source_note_id: intent.request.source_note_id,
            destination_note_id: intent.request.destination_note_id,
            block_bids: intent.blocks.iter().map(|block| block.bid).collect(),
        }
    }

    fn overlaps(&self, request: &BlockRelocationRequest) -> bool {
        let notes = [request.source_note_id, request.destination_note_id];
        notes.contains(&self.source_note_id)
            || notes.contains(&self.destination_note_id)
            || self.block_bids.contains(&request.root_bid)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelocationFailpoint {
    AfterPrepared,
    DuringDestinationAuthoring,
    AfterDestinationDurable,
    AfterSourceDurable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum RelocationPhase {
    Prepared,
    DestinationDurable,
    SourceDurable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum PersistedPropertyValue {
    Scalar(PropScalar),
    Text(String),
    List(Vec<PropScalar>),
}

impl From<ResolvedValue> for PersistedPropertyValue {
    fn from(value: ResolvedValue) -> Self {
        match value {
            ResolvedValue::Scalar(value) => Self::Scalar(value),
            ResolvedValue::Text(value) => Self::Text(value),
            ResolvedValue::List(values) => Self::List(values),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedRelocatedNoteVersion {
    note_id: [u8; 16],
    slug: String,
    pre_version: Vec<u8>,
    changed: bool,
    created: bool,
}

impl From<&RelocatedNoteVersion> for PersistedRelocatedNoteVersion {
    fn from(value: &RelocatedNoteVersion) -> Self {
        Self {
            note_id: value.note_id,
            slug: value.slug.clone(),
            pre_version: value.pre_version.clone(),
            changed: value.changed,
            created: value.created,
        }
    }
}

impl From<&PersistedRelocatedNoteVersion> for RelocatedNoteVersion {
    fn from(value: &PersistedRelocatedNoteVersion) -> Self {
        Self {
            note_id: value.note_id,
            slug: value.slug.clone(),
            pre_version: value.pre_version.clone(),
            changed: value.changed,
            created: value.created,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum PersistedRelocationStatus {
    Applied,
    NoOp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DestinationRootProof {
    move_id: [u8; 16],
    request_hash: [u8; 32],
    root_bid: [u8; 16],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RelocationIntent {
    request_hash: [u8; 32],
    request: BlockRelocationRequest,
    blocks: Vec<RelocatedBlockSnapshot>,
    destination_order_without_source: Vec<TreeID>,
    insertion_index: usize,
    new_indents: Vec<u16>,
    new_parents: Vec<Option<[u8; 16]>>,
    source_pre_version: Vec<u8>,
    destination_pre_version: Vec<u8>,
    destination_created: bool,
    no_op: bool,
    phase: RelocationPhase,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RelocationReceipt {
    move_id: [u8; 16],
    request_hash: [u8; 32],
    status: PersistedRelocationStatus,
    notes: Vec<PersistedRelocatedNoteVersion>,
    destination_root_proof: Option<DestinationRootProof>,
    completed_at: HlcTimestamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RelocationRecord {
    Intent(RelocationIntent),
    Receipt(RelocationReceipt),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RelocatedBlockSnapshot {
    source_node: TreeID,
    bid: [u8; 16],
    text: String,
    indent: u16,
    parent: Option<[u8; 16]>,
    props: Vec<(String, PersistedPropertyValue)>,
}

struct PreparedRelocation {
    request: BlockRelocationRequest,
    source_doc: LoroDoc,
    destination_doc: Option<LoroDoc>,
    destination_order_without_source: Vec<TreeID>,
    blocks: Vec<RelocatedBlockSnapshot>,
    insertion_index: usize,
    new_indents: Vec<u16>,
    new_parents: Vec<Option<[u8; 16]>>,
    source_pre_version: Vec<u8>,
    destination_pre_version: Vec<u8>,
    destination_created: bool,
    no_op: bool,
}

fn rejected(message: impl Into<String>) -> SyncError {
    SyncError::RelocationRejected(message.into())
}

fn request_hash(request: &BlockRelocationRequest) -> SyncResult<[u8; 32]> {
    let bytes = postcard::to_allocvec(request)
        .map_err(|error| SyncError::Storage(format!("encode relocation request: {error}")))?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

fn decode_fixed_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    let bytes = hex::decode(value).ok()?;
    bytes.try_into().ok()
}

fn recovery_required(move_id: [u8; 16], error: impl std::fmt::Display) -> SyncError {
    SyncError::RelocationRecoveryRequired {
        move_id,
        message: error.to_string(),
    }
}

fn preserve_recovery_error(move_id: [u8; 16], error: SyncError) -> SyncError {
    match error {
        SyncError::RelocationRecoveryRequired { .. } => error,
        other => recovery_required(move_id, other),
    }
}

impl RelocationIntent {
    fn from_prepared(prepared: &PreparedRelocation, request_hash: [u8; 32]) -> Self {
        Self {
            request_hash,
            request: prepared.request.clone(),
            blocks: prepared.blocks.clone(),
            destination_order_without_source: prepared.destination_order_without_source.clone(),
            insertion_index: prepared.insertion_index,
            new_indents: prepared.new_indents.clone(),
            new_parents: prepared.new_parents.clone(),
            source_pre_version: prepared.source_pre_version.clone(),
            destination_pre_version: prepared.destination_pre_version.clone(),
            destination_created: prepared.destination_created,
            no_op: prepared.no_op,
            phase: RelocationPhase::Prepared,
        }
    }

    fn outcome(&self, replayed: bool) -> BlockRelocationOutcome {
        let status = if replayed {
            BlockRelocationStatus::Replayed
        } else if self.no_op {
            BlockRelocationStatus::NoOp
        } else {
            BlockRelocationStatus::Applied
        };
        let same_note = self.request.source_note_id == self.request.destination_note_id;
        let mut notes = vec![RelocatedNoteVersion {
            note_id: self.request.source_note_id,
            slug: self.request.source_slug.clone(),
            pre_version: self.source_pre_version.clone(),
            changed: !self.no_op,
            created: false,
        }];
        if !same_note {
            notes.push(RelocatedNoteVersion {
                note_id: self.request.destination_note_id,
                slug: self.request.destination_slug.clone(),
                pre_version: self.destination_pre_version.clone(),
                changed: true,
                created: self.destination_created,
            });
        }
        BlockRelocationOutcome {
            move_id: self.request.move_id,
            status,
            notes,
        }
    }
}

impl RelocationReceipt {
    fn replay_outcome(&self) -> BlockRelocationOutcome {
        BlockRelocationOutcome {
            move_id: self.move_id,
            status: BlockRelocationStatus::Replayed,
            notes: self.notes.iter().map(Into::into).collect(),
        }
    }
}

fn live_root_nodes(tree: &LoroTree) -> Vec<TreeID> {
    tree.children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| !matches!(tree.is_node_deleted(node), Ok(true)))
        .collect()
}

fn node_bid(tree: &LoroTree, node: TreeID) -> Option<[u8; 16]> {
    read_meta_str(tree, node, "block_id").and_then(|value| parse_note_id_from_hex(&value))
}

fn note_slug(doc: &LoroDoc) -> Option<String> {
    doc.get_map("root")
        .get("slug")
        .and_then(|value| value.into_value().ok())
        .and_then(|value| value.into_string().ok())
        .map(|value| (*value).clone())
        .filter(|value| !value.is_empty())
}

fn validate_slug(doc: &LoroDoc, requested: &str, role: &str) -> SyncResult<()> {
    if note_slug(doc).is_some_and(|stored| stored != requested) {
        return Err(rejected(format!(
            "{role} note id does not match requested slug {requested}"
        )));
    }
    Ok(())
}

fn capture_subtree(
    source_doc: &LoroDoc,
    root_bid: [u8; 16],
) -> SyncResult<(Vec<TreeID>, Vec<RelocatedBlockSnapshot>)> {
    let tree = source_doc.get_tree("blocks");
    let source_order = live_root_nodes(&tree);
    let root_index = source_order
        .iter()
        .position(|node| node_bid(&tree, *node) == Some(root_bid))
        .ok_or_else(|| rejected(format!("missing source root {}", hex_id(&root_bid))))?;
    let root_indent = read_indent_level(&tree, source_order[root_index])
        .ok_or_else(|| rejected("source root has no valid indent metadata"))?;
    let mut subtree_end = source_order.len();
    for (offset, node) in source_order[root_index + 1..].iter().enumerate() {
        let indent = read_indent_level(&tree, *node)
            .ok_or_else(|| rejected("source subtree boundary contains invalid indent metadata"))?;
        if indent <= root_indent {
            subtree_end = root_index + 1 + offset;
            break;
        }
    }

    let mut blocks = Vec::with_capacity(subtree_end - root_index);
    for node in &source_order[root_index..subtree_end] {
        let bid = node_bid(&tree, *node)
            .ok_or_else(|| rejected("source subtree contains a block without a valid bid"))?;
        let indent = read_indent_level(&tree, *node)
            .ok_or_else(|| rejected(format!("block {} has no valid indent", hex_id(&bid))))?;
        let meta = tree
            .get_meta(*node)
            .map_err(|error| rejected(format!("read block {} metadata: {error}", hex_id(&bid))))?;
        let parent = match read_meta_str(&tree, *node, "parent") {
            Some(value) => Some(parse_note_id_from_hex(&value).ok_or_else(|| {
                rejected(format!(
                    "block {} has invalid parent metadata",
                    hex_id(&bid)
                ))
            })?),
            None => None,
        };
        let props = prop_containers::read_node_prop_containers(&meta)
            .map(|(props, prop_keys)| {
                prop_containers::read_props_typed(&props, &prop_keys)
                    .into_iter()
                    .map(|(key, value)| (key, value.into()))
                    .collect()
            })
            .unwrap_or_default();
        blocks.push(RelocatedBlockSnapshot {
            source_node: *node,
            bid,
            text: read_block_text(&tree, *node).unwrap_or_default(),
            indent,
            parent,
            props,
        });
    }
    Ok((source_order, blocks))
}

fn target_placement(
    tree: &LoroTree,
    order: &[TreeID],
    target_bid: Option<[u8; 16]>,
    placement: MovePlacement,
) -> SyncResult<(usize, u16, Option<[u8; 16]>)> {
    if placement == MovePlacement::Append {
        return Ok((order.len(), 0, None));
    }
    let target_bid = target_bid.ok_or_else(|| rejected("target bid is required"))?;
    let target_index = order
        .iter()
        .position(|node| node_bid(tree, *node) == Some(target_bid))
        .ok_or_else(|| {
            rejected(format!(
                "missing destination target {}",
                hex_id(&target_bid)
            ))
        })?;
    let target_node = order[target_index];
    let target_indent = read_indent_level(tree, target_node)
        .ok_or_else(|| rejected("destination target has no valid indent metadata"))?;
    let target_parent = match read_meta_str(tree, target_node, "parent") {
        Some(value) => Some(
            parse_note_id_from_hex(&value)
                .ok_or_else(|| rejected("destination target has invalid parent metadata"))?,
        ),
        None => None,
    };
    let mut after_subtree = order.len();
    for (offset, node) in order[target_index + 1..].iter().enumerate() {
        let indent = read_indent_level(tree, *node).ok_or_else(|| {
            rejected("destination target boundary contains invalid indent metadata")
        })?;
        if indent <= target_indent {
            after_subtree = target_index + 1 + offset;
            break;
        }
    }

    match placement {
        MovePlacement::Before => Ok((target_index, target_indent, target_parent)),
        MovePlacement::Inside => Ok((
            after_subtree,
            target_indent
                .checked_add(1)
                .ok_or_else(|| rejected("destination target indent is too deep"))?,
            Some(target_bid),
        )),
        MovePlacement::After => Ok((after_subtree, target_indent, target_parent)),
        MovePlacement::Append => unreachable!("append returned above"),
    }
}

fn final_order(
    destination_without_source: &[TreeID],
    insertion_index: usize,
    moved: &[RelocatedBlockSnapshot],
) -> Vec<TreeID> {
    let mut result = Vec::with_capacity(destination_without_source.len() + moved.len());
    result.extend_from_slice(&destination_without_source[..insertion_index]);
    result.extend(moved.iter().map(|block| block.source_node));
    result.extend_from_slice(&destination_without_source[insertion_index..]);
    result
}

fn destination_order_without_captured(
    tree: &LoroTree,
    blocks: &[RelocatedBlockSnapshot],
) -> Vec<TreeID> {
    let captured_bids: HashSet<[u8; 16]> = blocks.iter().map(|block| block.bid).collect();
    live_root_nodes(tree)
        .into_iter()
        .filter(|node| {
            node_bid(tree, *node)
                .map(|bid| !captured_bids.contains(&bid))
                .unwrap_or(true)
        })
        .collect()
}

fn captured_source_nodes_are_unique(tree: &LoroTree, blocks: &[RelocatedBlockSnapshot]) -> bool {
    let order = live_root_nodes(tree);
    blocks.iter().all(|block| {
        if !matches!(tree.is_node_deleted(&block.source_node), Ok(false)) {
            return false;
        }
        let mut matching = order
            .iter()
            .copied()
            .filter(|node| node_bid(tree, *node) == Some(block.bid));
        matching.next() == Some(block.source_node) && matching.next().is_none()
    })
}

struct CurrentDestinationPlacement {
    insertion_index: usize,
    new_indents: Vec<u16>,
    new_parents: Vec<Option<[u8; 16]>>,
}

fn relocated_snapshot_ancestry(
    blocks: &[RelocatedBlockSnapshot],
    new_root_indent: u16,
    new_root_parent: Option<[u8; 16]>,
) -> SyncResult<(Vec<u16>, Vec<Option<[u8; 16]>>)> {
    let old_root_indent = blocks
        .first()
        .ok_or_else(|| rejected("relocation subtree is empty"))?
        .indent;
    let mut new_indents = Vec::with_capacity(blocks.len());
    let mut new_parents = Vec::with_capacity(blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        let relative = block
            .indent
            .checked_sub(old_root_indent)
            .ok_or_else(|| rejected("source subtree indentation is inconsistent"))?;
        new_indents.push(
            new_root_indent
                .checked_add(relative)
                .ok_or_else(|| rejected("relocated subtree indentation is too deep"))?,
        );
        new_parents.push(if index == 0 {
            new_root_parent
        } else {
            block.parent
        });
    }
    Ok((new_indents, new_parents))
}

fn current_destination_placement(
    tree: &LoroTree,
    prepared: &PreparedRelocation,
    order_without_captured: &[TreeID],
) -> SyncResult<CurrentDestinationPlacement> {
    let (insertion_index, new_root_indent, new_root_parent) =
        if prepared.request.placement == MovePlacement::Append {
            (order_without_captured.len(), 0, None)
        } else {
            let target_bid = prepared
                .request
                .target_bid
                .ok_or_else(|| recovery_required(prepared.request.move_id, "target is missing"))?;
            if !order_without_captured
                .iter()
                .any(|node| node_bid(tree, *node) == Some(target_bid))
            {
                return Err(recovery_required(
                    prepared.request.move_id,
                    format!(
                        "destination target {} vanished during relocation recovery",
                        hex_id(&target_bid)
                    ),
                ));
            }
            target_placement(
                tree,
                order_without_captured,
                Some(target_bid),
                prepared.request.placement,
            )
            .map_err(|error| recovery_required(prepared.request.move_id, error))?
        };
    let (new_indents, new_parents) =
        relocated_snapshot_ancestry(&prepared.blocks, new_root_indent, new_root_parent)
            .map_err(|error| recovery_required(prepared.request.move_id, error))?;
    Ok(CurrentDestinationPlacement {
        insertion_index,
        new_indents,
        new_parents,
    })
}

fn apply_snapshot_metadata(
    tree: &LoroTree,
    node: TreeID,
    block: &RelocatedBlockSnapshot,
    indent: u16,
    parent: Option<[u8; 16]>,
) -> SyncResult<()> {
    let meta = tree
        .get_meta(node)
        .map_err(|error| SyncError::Storage(format!("loro relocation get_meta: {error}")))?;
    meta.insert("block_id", hex_id(&block.bid))
        .map_err(|error| SyncError::Storage(format!("loro relocation block id: {error}")))?;
    meta.insert("indent_level", indent as i64)
        .map_err(|error| SyncError::Storage(format!("loro relocation indent: {error}")))?;
    meta.insert(
        "parent",
        parent.map(|value| hex_id(&value)).unwrap_or_default(),
    )
    .map_err(|error| SyncError::Storage(format!("loro relocation parent: {error}")))?;
    Ok(())
}

fn persisted_properties(meta: &loro::LoroMap) -> Vec<(String, PersistedPropertyValue)> {
    prop_containers::read_node_prop_containers(meta)
        .map(|(props, prop_keys)| {
            prop_containers::read_props_typed(&props, &prop_keys)
                .into_iter()
                .map(|(key, value)| (key, value.into()))
                .collect()
        })
        .unwrap_or_default()
}

fn write_snapshot_properties(
    meta: &loro::LoroMap,
    properties: &[(String, PersistedPropertyValue)],
) -> SyncResult<()> {
    let (props, prop_keys) = prop_containers::node_prop_containers(meta)?;
    for (key, value) in properties {
        match value {
            PersistedPropertyValue::Scalar(value) => {
                prop_containers::prop_set_scalar(&props, &prop_keys, key, value)?;
            }
            PersistedPropertyValue::Text(value) => {
                prop_containers::prop_set_text(&props, &prop_keys, key, value)?;
            }
            PersistedPropertyValue::List(values) => {
                let _ = prop_containers::prop_ensure_list(&props, &prop_keys, key)?;
                for value in values {
                    prop_containers::prop_add_to_list(&props, &prop_keys, key, value)?;
                }
            }
        }
    }
    Ok(())
}

fn author_snapshot_block(
    tree: &LoroTree,
    node: TreeID,
    block: &RelocatedBlockSnapshot,
    indent: u16,
    parent: Option<[u8; 16]>,
) -> SyncResult<()> {
    apply_snapshot_metadata(tree, node, block, indent, parent)?;
    let meta = tree
        .get_meta(node)
        .map_err(|error| SyncError::Storage(format!("loro relocation get_meta: {error}")))?;
    write_block_text(&meta, &block.text)?;
    write_snapshot_properties(&meta, &block.props)
}

fn reconcile_snapshot_block(
    tree: &LoroTree,
    node: TreeID,
    block: &RelocatedBlockSnapshot,
    indent: u16,
    parent: Option<[u8; 16]>,
) -> SyncResult<()> {
    apply_snapshot_metadata(tree, node, block, indent, parent)?;
    let meta = tree
        .get_meta(node)
        .map_err(|error| SyncError::Storage(format!("loro relocation get_meta: {error}")))?;
    write_block_text(&meta, &block.text)?;
    let current = persisted_properties(&meta);
    if current != block.props {
        let (props, prop_keys) = prop_containers::node_prop_containers(&meta)?;
        for (key, _) in current {
            prop_containers::prop_clear(&props, &prop_keys, &key)?;
        }
        write_snapshot_properties(&meta, &block.props)?;
    }
    Ok(())
}

impl LoroEngine {
    fn relocation_tombstones_path(&self) -> Option<PathBuf> {
        Some(
            self.inner
                .snapshot_dir
                .as_ref()?
                .join(RELOCATION_TOMBSTONES_FILE),
        )
    }

    fn relocation_record_path(&self, move_id: [u8; 16]) -> Option<PathBuf> {
        Some(
            self.inner
                .snapshot_dir
                .as_ref()?
                .join(RELOCATION_DIR)
                .join(format!("{}.bin", hex_id(&move_id))),
        )
    }

    async fn persist_relocation_record(
        &self,
        move_id: [u8; 16],
        record: &RelocationRecord,
    ) -> SyncResult<()> {
        let Some(path) = self.relocation_record_path(move_id) else {
            return Ok(());
        };
        let parent = path
            .parent()
            .expect("relocation record path always has a parent");
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            SyncError::Storage(format!(
                "create relocation directory {}: {error}",
                parent.display()
            ))
        })?;
        let bytes = postcard::to_allocvec(record)
            .map_err(|error| SyncError::Storage(format!("encode relocation record: {error}")))?;
        let tmp = unique_tmp(&path);
        tokio::fs::write(&tmp, bytes).await.map_err(|error| {
            SyncError::Storage(format!(
                "write relocation record {}: {error}",
                tmp.display()
            ))
        })?;
        if let Err(error) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(SyncError::Storage(format!(
                "publish relocation record {}: {error}",
                path.display()
            )));
        }
        Ok(())
    }

    async fn read_relocation_record(
        &self,
        move_id: [u8; 16],
    ) -> SyncResult<Option<RelocationRecord>> {
        let Some(path) = self.relocation_record_path(move_id) else {
            return Ok(None);
        };
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(SyncError::Storage(format!(
                    "read relocation record {}: {error}",
                    path.display()
                )))
            }
        };
        postcard::from_bytes(&bytes).map(Some).map_err(|error| {
            recovery_required(move_id, format!("decode relocation record: {error}"))
        })
    }

    async fn scan_relocation_records(&self) -> SyncResult<Vec<RelocationRecord>> {
        let Some(snapshot_dir) = self.inner.snapshot_dir.as_ref() else {
            return Ok(Vec::new());
        };
        let dir = snapshot_dir.join(RELOCATION_DIR);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(SyncError::Storage(format!(
                    "read relocation directory {}: {error}",
                    dir.display()
                )))
            }
        };
        let mut records = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|error| {
            SyncError::Storage(format!(
                "scan relocation directory {}: {error}",
                dir.display()
            ))
        })? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("bin") {
                continue;
            }
            let move_id = path
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(decode_fixed_hex::<16>)
                .unwrap_or([0; 16]);
            let bytes = tokio::fs::read(&path).await.map_err(|error| {
                recovery_required(
                    move_id,
                    format!("read relocation record {}: {error}", path.display()),
                )
            })?;
            let record: RelocationRecord = postcard::from_bytes(&bytes).map_err(|error| {
                recovery_required(
                    move_id,
                    format!("decode relocation record {}: {error}", path.display()),
                )
            })?;
            let recorded_move_id = match &record {
                RelocationRecord::Intent(intent) => intent.request.move_id,
                RelocationRecord::Receipt(receipt) => receipt.move_id,
            };
            if recorded_move_id != move_id {
                return Err(recovery_required(
                    move_id,
                    "relocation filename does not match record move id",
                ));
            }
            records.push(record);
        }
        Ok(records)
    }

    async fn load_relocation_tombstones(&self) -> SyncResult<RelocationTombstones> {
        let Some(path) = self.relocation_tombstones_path() else {
            return Ok(Default::default());
        };
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Default::default())
            }
            Err(error) => {
                return Err(SyncError::Storage(format!(
                    "read relocation tombstones {}: {error}",
                    path.display()
                )))
            }
        };
        postcard::from_bytes(&bytes).map_err(|error| {
            SyncError::Storage(format!(
                "decode relocation tombstones {}: {error}",
                path.display()
            ))
        })
    }

    async fn publish_relocation_tombstones(
        &self,
        tombstones: &RelocationTombstones,
    ) -> SyncResult<()> {
        let Some(path) = self.relocation_tombstones_path() else {
            return Ok(());
        };
        let bytes = postcard::to_allocvec(tombstones).map_err(|error| {
            SyncError::Storage(format!("encode relocation tombstones: {error}"))
        })?;
        let tmp = unique_tmp(&path);
        tokio::fs::write(&tmp, bytes).await.map_err(|error| {
            SyncError::Storage(format!(
                "write relocation tombstones {}: {error}",
                tmp.display()
            ))
        })?;
        if let Err(error) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(SyncError::Storage(format!(
                "publish relocation tombstones {}: {error}",
                path.display()
            )));
        }
        Ok(())
    }

    async fn persist_relocation_tombstone(
        &self,
        move_id: [u8; 16],
        hash: [u8; 32],
    ) -> SyncResult<()> {
        let mut tombstones = self.inner.relocation_tombstones.lock().await;
        if let Some(existing) = tombstones.get(&move_id) {
            return if *existing == hash {
                Ok(())
            } else {
                Err(SyncError::RelocationConflict(
                    "move id was already completed with different arguments".into(),
                ))
            };
        }
        let mut updated = tombstones.clone();
        updated.insert(move_id, hash);
        self.publish_relocation_tombstones(&updated).await?;
        *tombstones = updated;
        Ok(())
    }

    async fn persist_receipt(&self, receipt: RelocationReceipt) -> SyncResult<()> {
        self.persist_relocation_record(
            receipt.move_id,
            &RelocationRecord::Receipt(receipt.clone()),
        )
        .await?;
        self.inner
            .active_relocations
            .lock()
            .await
            .remove(&receipt.move_id);
        if self.inner.snapshot_dir.is_none() {
            return Ok(());
        }
        let mut index = self.inner.relocation_receipts.lock().await;
        index.insert((receipt.completed_at, receipt.move_id), ());
        let mut pruned = Vec::new();
        while index.len() > RECEIPT_LIMIT {
            let Some(((_, move_id), _)) = index.pop_first() else {
                break;
            };
            pruned.push(move_id);
        }
        drop(index);
        for move_id in pruned {
            let Some(path) = self.relocation_record_path(move_id) else {
                continue;
            };
            if let Err(error) = tokio::fs::remove_file(&path).await {
                if error.kind() != std::io::ErrorKind::NotFound {
                    return Err(SyncError::Storage(format!(
                        "prune relocation receipt {}: {error}",
                        path.display()
                    )));
                }
            }
        }
        Ok(())
    }

    async fn persist_prepared_intent(&self, intent: &RelocationIntent) -> SyncResult<()> {
        let move_id = intent.request.move_id;
        let mut active = self.inner.active_relocations.lock().await;
        active.insert(move_id, RelocationReservation::from_intent(intent));
        if let Err(error) = self
            .persist_relocation_record(move_id, &RelocationRecord::Intent(intent.clone()))
            .await
        {
            active.remove(&move_id);
            return Err(error);
        }
        Ok(())
    }

    async fn overlapping_pending_move(&self, request: &BlockRelocationRequest) -> Option<[u8; 16]> {
        self.inner
            .active_relocations
            .lock()
            .await
            .iter()
            .find_map(|(move_id, reservation)| {
                (*move_id != request.move_id && reservation.overlaps(request)).then_some(*move_id)
            })
    }

    #[cfg(test)]
    pub(super) async fn inject_relocation_failure_once(&self, failpoint: RelocationFailpoint) {
        *self.inner.relocation_failpoint.lock().await = Some(failpoint);
    }

    #[cfg(test)]
    async fn fail_relocation_at(
        &self,
        expected: RelocationFailpoint,
        move_id: [u8; 16],
    ) -> SyncResult<()> {
        let mut failpoint = self.inner.relocation_failpoint.lock().await;
        if *failpoint == Some(expected) {
            *failpoint = None;
            return Err(recovery_required(
                move_id,
                format!("injected failure at {expected:?}"),
            ));
        }
        Ok(())
    }

    async fn checkpoint_after_prepared(&self, _move_id: [u8; 16]) -> SyncResult<()> {
        #[cfg(test)]
        self.fail_relocation_at(RelocationFailpoint::AfterPrepared, _move_id)
            .await?;
        Ok(())
    }

    async fn checkpoint_during_destination_authoring(&self, _move_id: [u8; 16]) -> SyncResult<()> {
        #[cfg(test)]
        self.fail_relocation_at(RelocationFailpoint::DuringDestinationAuthoring, _move_id)
            .await?;
        Ok(())
    }

    async fn checkpoint_after_destination_durable(&self, _move_id: [u8; 16]) -> SyncResult<()> {
        #[cfg(test)]
        self.fail_relocation_at(RelocationFailpoint::AfterDestinationDurable, _move_id)
            .await?;
        Ok(())
    }

    async fn checkpoint_after_source_durable(&self, _move_id: [u8; 16]) -> SyncResult<()> {
        #[cfg(test)]
        self.fail_relocation_at(RelocationFailpoint::AfterSourceDurable, _move_id)
            .await?;
        Ok(())
    }

    fn read_destination_proof_at(tree: &LoroTree, node: TreeID) -> Option<DestinationRootProof> {
        Some(DestinationRootProof {
            move_id: decode_fixed_hex(&read_meta_str(tree, node, RELOCATION_MOVE_ID_META)?)?,
            request_hash: decode_fixed_hex(&read_meta_str(
                tree,
                node,
                RELOCATION_REQUEST_HASH_META,
            )?)?,
            root_bid: node_bid(tree, node)?,
        })
    }

    fn write_destination_proof(
        tree: &LoroTree,
        node: TreeID,
        move_id: [u8; 16],
        request_hash: [u8; 32],
    ) -> SyncResult<()> {
        let meta = tree.get_meta(node).map_err(|error| {
            SyncError::Storage(format!("loro relocation proof metadata: {error}"))
        })?;
        meta.insert(RELOCATION_MOVE_ID_META, hex_id(&move_id))
            .map_err(|error| SyncError::Storage(format!("loro relocation move proof: {error}")))?;
        meta.insert(RELOCATION_REQUEST_HASH_META, hex::encode(request_hash))
            .map_err(|error| SyncError::Storage(format!("loro relocation hash proof: {error}")))?;
        Ok(())
    }

    async fn completed_proof_for_move(&self, move_id: [u8; 16]) -> Option<DestinationRootProof> {
        let docs = self.inner.docs.read().await;
        for doc in docs.values() {
            let tree = doc.get_tree("blocks");
            for node in live_root_nodes(&tree) {
                let Some(proof) = Self::read_destination_proof_at(&tree, node) else {
                    continue;
                };
                if proof.move_id == move_id {
                    return Some(proof);
                }
            }
        }
        None
    }

    async fn metadata_replay_outcome(
        &self,
        request: &BlockRelocationRequest,
    ) -> BlockRelocationOutcome {
        let same_note = request.source_note_id == request.destination_note_id;
        let mut notes = vec![RelocatedNoteVersion {
            note_id: request.source_note_id,
            slug: request.source_slug.clone(),
            pre_version: self
                .doc_version(request.source_note_id)
                .await
                .unwrap_or_default(),
            changed: false,
            created: false,
        }];
        if !same_note {
            notes.push(RelocatedNoteVersion {
                note_id: request.destination_note_id,
                slug: request.destination_slug.clone(),
                pre_version: self
                    .doc_version(request.destination_note_id)
                    .await
                    .unwrap_or_default(),
                changed: false,
                created: false,
            });
        }
        BlockRelocationOutcome {
            move_id: request.move_id,
            status: BlockRelocationStatus::Replayed,
            notes,
        }
    }

    fn destination_snapshot_complete(
        &self,
        prepared: &PreparedRelocation,
        request_hash: [u8; 32],
    ) -> SyncResult<bool> {
        let same_note = prepared.request.source_note_id == prepared.request.destination_note_id;
        let Some(doc) = prepared.destination_doc.as_ref() else {
            return Ok(false);
        };
        let tree = doc.get_tree("blocks");
        let Some(root_node) = find_node_by_block_id(&tree, &hex_id(&prepared.request.root_bid))
        else {
            return Ok(false);
        };
        let Some(proof) = Self::read_destination_proof_at(&tree, root_node) else {
            return Ok(false);
        };
        if proof.move_id != prepared.request.move_id || proof.request_hash != request_hash {
            return Err(SyncError::RelocationConflict(
                "destination root carries different relocation metadata".into(),
            ));
        }
        let order = live_root_nodes(&tree);
        let Some(actual_start) = order.iter().position(|node| *node == root_node) else {
            return Ok(false);
        };
        if actual_start + prepared.blocks.len() > order.len() {
            return Ok(false);
        }
        for block in &prepared.blocks {
            if order
                .iter()
                .filter(|node| node_bid(&tree, **node) == Some(block.bid))
                .count()
                != 1
            {
                return Ok(false);
            }
        }
        let order_without_captured = destination_order_without_captured(&tree, &prepared.blocks);
        let placement = current_destination_placement(&tree, prepared, &order_without_captured)?;
        if actual_start != placement.insertion_index {
            return Ok(false);
        }
        for (offset, block) in prepared.blocks.iter().enumerate() {
            let node = order[actual_start + offset];
            if node_bid(&tree, node) != Some(block.bid)
                || read_block_text(&tree, node).as_deref() != Some(block.text.as_str())
                || read_indent_level(&tree, node) != Some(placement.new_indents[offset])
                || read_meta_str(&tree, node, "parent")
                    .and_then(|value| parse_note_id_from_hex(&value))
                    != placement.new_parents[offset]
            {
                return Ok(false);
            }
            let meta = tree.get_meta(node).map_err(|error| {
                recovery_required(
                    prepared.request.move_id,
                    format!("read destination block metadata: {error}"),
                )
            })?;
            let props = persisted_properties(&meta);
            if props != block.props {
                return Ok(false);
            }
        }
        if same_note && node_bid(&tree, root_node) != Some(prepared.request.root_bid) {
            return Ok(false);
        }
        Ok(true)
    }

    async fn persist_note_boundary(&self, note_id: [u8; 16]) -> SyncResult<()> {
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot_checked(dir, note_id).await?;
        }
        self.materialize_note_checked(note_id).await
    }

    async fn relocation_owner(&self, bid: &[u8; 16]) -> SyncResult<Option<[u8; 16]>> {
        self.unique_note_for_block(bid)
            .await
            .map_err(|error| rejected(error.to_string()))
    }

    async fn validate_owner(
        &self,
        bid: &[u8; 16],
        expected_note: [u8; 16],
        role: &str,
    ) -> SyncResult<()> {
        match self.relocation_owner(bid).await? {
            Some(note_id) if note_id == expected_note => Ok(()),
            Some(note_id) => Err(rejected(format!(
                "{role} {} belongs to note {}, not {}",
                hex_id(bid),
                hex_id(&note_id),
                hex_id(&expected_note)
            ))),
            None => Err(rejected(format!("missing {role} {}", hex_id(bid)))),
        }
    }

    async fn prepare_relocation_under_locks(
        &self,
        request: BlockRelocationRequest,
    ) -> SyncResult<PreparedRelocation> {
        match (request.placement, request.target_bid) {
            (MovePlacement::Append, None) => {}
            (MovePlacement::Append, Some(_)) => {
                return Err(rejected("append placement must not include a target bid"));
            }
            (_, Some(_)) => {}
            (_, None) => return Err(rejected("target-relative placement requires a target bid")),
        }
        if request.source_note_id == VIEWS_DOC_ID || request.destination_note_id == VIEWS_DOC_ID {
            return Err(rejected(
                "the views registry cannot participate in relocation",
            ));
        }
        if request.source_note_id == request.destination_note_id
            && request.source_slug != request.destination_slug
        {
            return Err(rejected("one note id cannot carry two relocation slugs"));
        }

        self.validate_owner(&request.root_bid, request.source_note_id, "source root")
            .await?;
        let source_doc = self
            .lazy_load_doc(request.source_note_id)
            .await
            .ok_or_else(|| rejected("source note is missing"))?;
        validate_slug(&source_doc, &request.source_slug, "source")?;
        let (source_order, blocks) = capture_subtree(&source_doc, request.root_bid)?;
        for block in &blocks {
            self.validate_owner(&block.bid, request.source_note_id, "source subtree block")
                .await?;
        }

        let same_note = request.source_note_id == request.destination_note_id;
        let destination_doc = if same_note {
            if request.destination_seed.is_some() {
                return Err(rejected(
                    "an existing destination cannot include a note seed",
                ));
            }
            Some(source_doc.clone())
        } else {
            self.lazy_load_doc(request.destination_note_id).await
        };
        if let Some(doc) = destination_doc.as_ref() {
            validate_slug(doc, &request.destination_slug, "destination")?;
            if !same_note && request.destination_seed.is_some() {
                return Err(rejected(
                    "an existing destination cannot include a note seed",
                ));
            }
        } else if request.destination_seed.is_none() {
            return Err(rejected(
                "destination note is missing and no seed was supplied",
            ));
        }

        if let Some(target_bid) = request.target_bid {
            self.validate_owner(
                &target_bid,
                request.destination_note_id,
                "destination target",
            )
            .await?;
            if blocks.iter().any(|block| block.bid == target_bid) {
                return Err(rejected(
                    "the destination target is the source root or its descendant",
                ));
            }
        }

        let moved_nodes: HashSet<TreeID> = blocks.iter().map(|block| block.source_node).collect();
        let destination_order_without_source = if same_note {
            source_order
                .iter()
                .copied()
                .filter(|node| !moved_nodes.contains(node))
                .collect()
        } else {
            destination_doc
                .as_ref()
                .map(|doc| live_root_nodes(&doc.get_tree("blocks")))
                .unwrap_or_default()
        };
        let placement_tree = destination_doc.as_ref().map(|doc| doc.get_tree("blocks"));
        let (insertion_index, new_root_indent, new_root_parent) = match placement_tree.as_ref() {
            Some(tree) => target_placement(
                tree,
                &destination_order_without_source,
                request.target_bid,
                request.placement,
            )?,
            None => {
                if request.placement != MovePlacement::Append {
                    return Err(rejected("a missing destination only supports append"));
                }
                (0, 0, None)
            }
        };

        let (new_indents, new_parents) =
            relocated_snapshot_ancestry(&blocks, new_root_indent, new_root_parent)?;

        let source_pre_version = source_doc.oplog_vv().encode();
        let destination_pre_version = if same_note {
            source_pre_version.clone()
        } else {
            destination_doc
                .as_ref()
                .map(|doc| doc.oplog_vv().encode())
                .unwrap_or_default()
        };
        let no_op = same_note
            && final_order(&destination_order_without_source, insertion_index, &blocks)
                == source_order
            && blocks.iter().enumerate().all(|(index, block)| {
                block.indent == new_indents[index] && block.parent == new_parents[index]
            });

        Ok(PreparedRelocation {
            request,
            source_doc,
            destination_doc,
            destination_order_without_source,
            blocks,
            insertion_index,
            new_indents,
            new_parents,
            source_pre_version,
            destination_pre_version,
            destination_created: !same_note && placement_tree.is_none(),
            no_op,
        })
    }

    fn seeded_destination_doc(&self, request: &BlockRelocationRequest) -> SyncResult<LoroDoc> {
        let seed = request
            .destination_seed
            .as_ref()
            .ok_or_else(|| rejected("destination note seed is missing"))?;
        let parsed = tesela_core::note_tree::parse_note(&seed.content);
        let doc = LoroDoc::new();
        self.set_doc_peer(&doc);
        let root = doc.get_map("root");
        root.insert(
            "frontmatter",
            parsed.frontmatter.as_deref().unwrap_or_default(),
        )
        .map_err(|error| SyncError::Storage(format!("loro relocation frontmatter: {error}")))?;
        root.insert(
            "slug",
            seed.display_alias
                .as_deref()
                .unwrap_or(&request.destination_slug),
        )
        .map_err(|error| SyncError::Storage(format!("loro relocation slug: {error}")))?;
        root.insert("title", seed.title.as_str())
            .map_err(|error| SyncError::Storage(format!("loro relocation title: {error}")))?;
        set_page_properties(&doc, &parsed.page_properties)?;
        Ok(doc)
    }

    fn apply_same_note_relocation(
        &self,
        prepared: &PreparedRelocation,
        request_hash: [u8; 32],
    ) -> SyncResult<()> {
        let tree = prepared.source_doc.get_tree("blocks");
        let captured_nodes_are_live_and_unique =
            captured_source_nodes_are_unique(&tree, &prepared.blocks);
        let order_without_captured = destination_order_without_captured(&tree, &prepared.blocks);
        let placement = current_destination_placement(&tree, prepared, &order_without_captured)?;
        let destination_root = if captured_nodes_are_live_and_unique {
            if placement.insertion_index < order_without_captured.len() {
                let anchor = order_without_captured[placement.insertion_index];
                for block in &prepared.blocks {
                    tree.mov_before(block.source_node, anchor)
                        .map_err(|error| {
                            SyncError::Storage(format!("loro relocation move before: {error}"))
                        })?;
                }
            } else if let Some(mut anchor) = order_without_captured.last().copied() {
                for block in &prepared.blocks {
                    tree.mov_after(block.source_node, anchor).map_err(|error| {
                        SyncError::Storage(format!("loro relocation move after: {error}"))
                    })?;
                    anchor = block.source_node;
                }
            } else {
                for (offset, block) in prepared.blocks.iter().enumerate() {
                    tree.mov_to(
                        block.source_node,
                        TreeParentId::Root,
                        placement.insertion_index + offset,
                    )
                    .map_err(|error| {
                        SyncError::Storage(format!("loro relocation anchorless move: {error}"))
                    })?;
                }
            }
            for (index, block) in prepared.blocks.iter().enumerate() {
                reconcile_snapshot_block(
                    &tree,
                    block.source_node,
                    block,
                    placement.new_indents[index],
                    placement.new_parents[index],
                )?;
            }
            prepared.blocks[0].source_node
        } else {
            self.delete_captured_destination_nodes(&tree, &prepared.blocks)?;
            let mut destination_root = None;
            for (offset, block) in prepared.blocks.iter().enumerate() {
                let node = tree
                    .create_at(TreeParentId::Root, placement.insertion_index + offset)
                    .map_err(|error| {
                        SyncError::Storage(format!("loro relocation same-note rebuild: {error}"))
                    })?;
                author_snapshot_block(
                    &tree,
                    node,
                    block,
                    placement.new_indents[offset],
                    placement.new_parents[offset],
                )?;
                destination_root.get_or_insert(node);
            }
            destination_root.ok_or_else(|| {
                recovery_required(
                    prepared.request.move_id,
                    "captured relocation subtree is empty",
                )
            })?
        };
        Self::write_destination_proof(
            &tree,
            destination_root,
            prepared.request.move_id,
            request_hash,
        )?;
        prepared.source_doc.commit();
        Ok(())
    }

    async fn apply_cross_note_destination(
        &self,
        prepared: &PreparedRelocation,
        request_hash: [u8; 32],
    ) -> SyncResult<LoroDoc> {
        let destination_doc = match prepared.destination_doc.as_ref() {
            Some(doc) => doc.clone(),
            None => self.seeded_destination_doc(&prepared.request)?,
        };
        let destination_tree = destination_doc.get_tree("blocks");
        let order_without_captured =
            destination_order_without_captured(&destination_tree, &prepared.blocks);
        let placement =
            current_destination_placement(&destination_tree, prepared, &order_without_captured)?;
        self.delete_captured_destination_nodes(&destination_tree, &prepared.blocks)?;
        for (offset, block) in prepared.blocks.iter().enumerate() {
            let node = destination_tree
                .create_at(TreeParentId::Root, placement.insertion_index + offset)
                .map_err(|error| {
                    SyncError::Storage(format!("loro relocation destination create: {error}"))
                })?;
            author_snapshot_block(
                &destination_tree,
                node,
                block,
                placement.new_indents[offset],
                placement.new_parents[offset],
            )?;
            self.checkpoint_during_destination_authoring(prepared.request.move_id)
                .await?;
        }
        let destination_root =
            find_node_by_block_id(&destination_tree, &hex_id(&prepared.request.root_bid))
                .ok_or_else(|| {
                    SyncError::Storage("relocation destination root was not authored".into())
                })?;
        Self::write_destination_proof(
            &destination_tree,
            destination_root,
            prepared.request.move_id,
            request_hash,
        )?;
        destination_doc.commit();
        if prepared.destination_created {
            self.inner.docs.write().await.insert(
                prepared.request.destination_note_id,
                destination_doc.clone(),
            );
        }
        Ok(destination_doc)
    }

    fn delete_captured_destination_nodes(
        &self,
        tree: &LoroTree,
        blocks: &[RelocatedBlockSnapshot],
    ) -> SyncResult<()> {
        let captured_bids: HashSet<[u8; 16]> = blocks.iter().map(|block| block.bid).collect();
        let captured_nodes: Vec<TreeID> = live_root_nodes(tree)
            .into_iter()
            .filter(|node| {
                node_bid(tree, *node)
                    .map(|bid| captured_bids.contains(&bid))
                    .unwrap_or(false)
            })
            .collect();
        for node in captured_nodes {
            tree.delete(node).map_err(|error| {
                SyncError::Storage(format!("loro relocation destination reconcile: {error}"))
            })?;
        }
        Ok(())
    }

    async fn ensure_destination_snapshot(
        &self,
        prepared: &mut PreparedRelocation,
        request_hash: [u8; 32],
    ) -> SyncResult<bool> {
        if self.destination_snapshot_complete(prepared, request_hash)? {
            return Ok(false);
        }
        let same_note = prepared.request.source_note_id == prepared.request.destination_note_id;
        if same_note {
            self.apply_same_note_relocation(prepared, request_hash)?;
            prepared.destination_doc = Some(prepared.source_doc.clone());
        } else {
            let destination_doc = self
                .apply_cross_note_destination(prepared, request_hash)
                .await?;
            prepared.destination_doc = Some(destination_doc);
        }
        if !self.destination_snapshot_complete(prepared, request_hash)? {
            return Err(recovery_required(
                prepared.request.move_id,
                "destination subtree reconciliation did not restore the captured snapshot",
            ));
        }
        Ok(true)
    }

    fn captured_source_deleted(&self, prepared: &PreparedRelocation) -> SyncResult<bool> {
        let tree = prepared.source_doc.get_tree("blocks");
        for block in &prepared.blocks {
            match tree.is_node_deleted(&block.source_node) {
                Ok(true) => {}
                Ok(false) => return Ok(false),
                Err(error) => {
                    return Err(recovery_required(
                        prepared.request.move_id,
                        format!("inspect captured source node deletion: {error}"),
                    ))
                }
            }
        }
        Ok(true)
    }

    fn delete_captured_source(&self, prepared: &PreparedRelocation) -> SyncResult<()> {
        let source_tree = prepared.source_doc.get_tree("blocks");
        for block in &prepared.blocks {
            if matches!(source_tree.is_node_deleted(&block.source_node), Ok(true)) {
                continue;
            }
            source_tree.delete(block.source_node).map_err(|error| {
                SyncError::Storage(format!("loro relocation source delete: {error}"))
            })?;
        }
        prepared.source_doc.commit();
        Ok(())
    }

    async fn prepared_from_intent(
        &self,
        intent: &RelocationIntent,
    ) -> SyncResult<PreparedRelocation> {
        let source_doc = self
            .lazy_load_doc(intent.request.source_note_id)
            .await
            .ok_or_else(|| {
                recovery_required(intent.request.move_id, "source note snapshot is missing")
            })?;
        let same_note = intent.request.source_note_id == intent.request.destination_note_id;
        let destination_doc = if same_note {
            Some(source_doc.clone())
        } else {
            self.lazy_load_doc(intent.request.destination_note_id).await
        };
        if intent.phase != RelocationPhase::Prepared && destination_doc.is_none() {
            return Err(recovery_required(
                intent.request.move_id,
                "durable destination snapshot is missing",
            ));
        }
        Ok(PreparedRelocation {
            request: intent.request.clone(),
            source_doc,
            destination_doc,
            destination_order_without_source: intent.destination_order_without_source.clone(),
            blocks: intent.blocks.clone(),
            insertion_index: intent.insertion_index,
            new_indents: intent.new_indents.clone(),
            new_parents: intent.new_parents.clone(),
            source_pre_version: intent.source_pre_version.clone(),
            destination_pre_version: intent.destination_pre_version.clone(),
            destination_created: intent.destination_created,
            no_op: intent.no_op,
        })
    }

    async fn complete_intent_under_locks(
        &self,
        mut intent: RelocationIntent,
        replayed: bool,
    ) -> SyncResult<BlockRelocationOutcome> {
        let move_id = intent.request.move_id;
        if intent.no_op {
            let outcome = intent.outcome(replayed);
            let receipt = RelocationReceipt {
                move_id,
                request_hash: intent.request_hash,
                status: PersistedRelocationStatus::NoOp,
                notes: outcome
                    .notes
                    .iter()
                    .map(PersistedRelocatedNoteVersion::from)
                    .collect(),
                destination_root_proof: None,
                completed_at: self.inner.hlc.now(),
            };
            self.persist_relocation_tombstone(move_id, intent.request_hash)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            self.persist_receipt(receipt)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            return Ok(outcome);
        }

        let mut prepared = self.prepared_from_intent(&intent).await?;
        let same_note = intent.request.source_note_id == intent.request.destination_note_id;
        if intent.phase == RelocationPhase::Prepared {
            self.ensure_destination_snapshot(&mut prepared, intent.request_hash)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            self.persist_note_boundary(intent.request.destination_note_id)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            intent.phase = RelocationPhase::DestinationDurable;
            self.persist_relocation_record(move_id, &RelocationRecord::Intent(intent.clone()))
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            self.checkpoint_after_destination_durable(move_id).await?;
        }

        if intent.phase == RelocationPhase::DestinationDurable {
            if self
                .ensure_destination_snapshot(&mut prepared, intent.request_hash)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?
            {
                self.persist_note_boundary(intent.request.destination_note_id)
                    .await
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
            }
            if !same_note {
                self.delete_captured_source(&prepared)
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
                self.persist_note_boundary(intent.request.source_note_id)
                    .await
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
                if !self
                    .captured_source_deleted(&prepared)
                    .map_err(|error| preserve_recovery_error(move_id, error))?
                {
                    return Err(recovery_required(
                        move_id,
                        "captured source nodes remain live after checked deletion",
                    ));
                }
            }
            intent.phase = RelocationPhase::SourceDurable;
            self.persist_relocation_record(move_id, &RelocationRecord::Intent(intent.clone()))
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
            self.checkpoint_after_source_durable(move_id).await?;
        }

        if intent.phase == RelocationPhase::SourceDurable {
            if self
                .ensure_destination_snapshot(&mut prepared, intent.request_hash)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?
            {
                self.persist_note_boundary(intent.request.destination_note_id)
                    .await
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
            }
            if !same_note
                && !self
                    .captured_source_deleted(&prepared)
                    .map_err(|error| preserve_recovery_error(move_id, error))?
            {
                self.delete_captured_source(&prepared)
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
                self.persist_note_boundary(intent.request.source_note_id)
                    .await
                    .map_err(|error| preserve_recovery_error(move_id, error))?;
            }
            if !same_note
                && !self
                    .captured_source_deleted(&prepared)
                    .map_err(|error| preserve_recovery_error(move_id, error))?
            {
                return Err(recovery_required(
                    move_id,
                    "captured source nodes remain live before receipt finalization",
                ));
            }
        }

        self.refresh_note_derived_under_ownership(
            intent.request.source_note_id,
            &prepared.source_doc,
        )
        .await;
        let destination_doc = if same_note {
            prepared.source_doc.clone()
        } else {
            prepared.destination_doc.clone().ok_or_else(|| {
                recovery_required(move_id, "destination doc vanished during completion")
            })?
        };
        if !same_note {
            self.refresh_note_derived_under_ownership(
                intent.request.destination_note_id,
                &destination_doc,
            )
            .await;
        }
        let outcome = intent.outcome(replayed);
        let receipt = RelocationReceipt {
            move_id,
            request_hash: intent.request_hash,
            status: PersistedRelocationStatus::Applied,
            notes: outcome
                .notes
                .iter()
                .map(PersistedRelocatedNoteVersion::from)
                .collect(),
            destination_root_proof: Some(DestinationRootProof {
                move_id,
                request_hash: intent.request_hash,
                root_bid: intent.request.root_bid,
            }),
            completed_at: self.inner.hlc.now(),
        };
        self.persist_relocation_tombstone(move_id, intent.request_hash)
            .await
            .map_err(|error| preserve_recovery_error(move_id, error))?;
        self.persist_receipt(receipt)
            .await
            .map_err(|error| preserve_recovery_error(move_id, error))?;
        Ok(outcome)
    }

    async fn relocate_under_locks(
        &self,
        request: BlockRelocationRequest,
    ) -> SyncResult<BlockRelocationOutcome> {
        let hash = request_hash(&request)?;
        if let Some(record) = self.read_relocation_record(request.move_id).await? {
            return match record {
                RelocationRecord::Receipt(receipt) => {
                    if receipt.request_hash != hash {
                        Err(SyncError::RelocationConflict(
                            "move id was already completed with different arguments".into(),
                        ))
                    } else {
                        Ok(receipt.replay_outcome())
                    }
                }
                RelocationRecord::Intent(intent) => {
                    if intent.request_hash != hash {
                        Err(SyncError::RelocationConflict(
                            "move id has a pending relocation with different arguments".into(),
                        ))
                    } else {
                        self.complete_intent_under_locks(intent, true).await
                    }
                }
            };
        }
        if let Some(completed_hash) = self
            .inner
            .relocation_tombstones
            .lock()
            .await
            .get(&request.move_id)
            .copied()
        {
            if completed_hash != hash {
                return Err(SyncError::RelocationConflict(
                    "move id was already completed with different arguments".into(),
                ));
            }
            return Err(rejected(
                "relocation receipt was pruned; move id is stale and cannot be replayed",
            ));
        }
        if let Some(pending_move_id) = self.overlapping_pending_move(&request).await {
            return Err(recovery_required(
                pending_move_id,
                "another relocation touching these notes requires recovery first",
            ));
        }
        if let Some(proof) = self.completed_proof_for_move(request.move_id).await {
            if proof.request_hash != hash || proof.root_bid != request.root_bid {
                return Err(SyncError::RelocationConflict(
                    "move id metadata belongs to different relocation arguments".into(),
                ));
            }
            return Ok(self.metadata_replay_outcome(&request).await);
        }

        let prepared = self.prepare_relocation_under_locks(request).await?;
        let intent = RelocationIntent::from_prepared(&prepared, hash);
        self.persist_prepared_intent(&intent).await?;
        self.checkpoint_after_prepared(intent.request.move_id)
            .await?;
        self.complete_intent_under_locks(intent, false).await
    }

    async fn recover_intent_with_locks(&self, intent: RelocationIntent) -> SyncResult<()> {
        let request = &intent.request;
        let source_lock = self.apply_lock_for_note(request.source_note_id).await;
        let destination_lock = if request.destination_note_id == request.source_note_id {
            None
        } else {
            Some(self.apply_lock_for_note(request.destination_note_id).await)
        };
        let source_first = request.source_note_id <= request.destination_note_id;

        if source_first {
            let _source_guard = source_lock.lock().await;
            let _destination_guard = match destination_lock.as_ref() {
                Some(lock) => Some(lock.lock().await),
                None => None,
            };
            let _ownership_guard = self.inner.ownership_transition.lock().await;
            self.complete_intent_under_locks(intent, true).await?;
        } else {
            let destination_lock = destination_lock
                .as_ref()
                .expect("different note ids always have a destination lock");
            let _destination_guard = destination_lock.lock().await;
            let _source_guard = source_lock.lock().await;
            let _ownership_guard = self.inner.ownership_transition.lock().await;
            self.complete_intent_under_locks(intent, true).await?;
        }
        Ok(())
    }

    pub(super) async fn recover_persisted_relocations(&self) -> SyncResult<()> {
        let records = self.scan_relocation_records().await?;
        let mut tombstones = self.load_relocation_tombstones().await?;
        let mut tombstones_changed = false;
        for record in &records {
            let (move_id, hash, completed) = match record {
                RelocationRecord::Intent(intent) => {
                    (intent.request.move_id, intent.request_hash, false)
                }
                RelocationRecord::Receipt(receipt) => (receipt.move_id, receipt.request_hash, true),
            };
            match tombstones.get(&move_id) {
                Some(existing) if *existing != hash => {
                    return Err(recovery_required(
                        move_id,
                        "relocation tombstone hash differs from durable record",
                    ));
                }
                Some(_) => {}
                None if completed => {
                    tombstones.insert(move_id, hash);
                    tombstones_changed = true;
                }
                None => {}
            }
        }
        if tombstones_changed {
            self.publish_relocation_tombstones(&tombstones).await?;
        }
        *self.inner.relocation_tombstones.lock().await = tombstones;
        {
            let mut receipts = self.inner.relocation_receipts.lock().await;
            let mut active = self.inner.active_relocations.lock().await;
            receipts.clear();
            active.clear();
            for record in &records {
                match record {
                    RelocationRecord::Receipt(receipt) => {
                        receipts.insert((receipt.completed_at, receipt.move_id), ());
                    }
                    RelocationRecord::Intent(intent) => {
                        active.insert(
                            intent.request.move_id,
                            RelocationReservation::from_intent(intent),
                        );
                    }
                }
            }
        }
        let mut intents: Vec<RelocationIntent> = records
            .into_iter()
            .filter_map(|record| match record {
                RelocationRecord::Intent(intent) => Some(intent),
                RelocationRecord::Receipt(_) => None,
            })
            .collect();
        intents.sort_by_key(|intent| intent.request.move_id);
        for intent in intents {
            let move_id = intent.request.move_id;
            self.recover_intent_with_locks(intent)
                .await
                .map_err(|error| preserve_recovery_error(move_id, error))?;
        }
        let mut index = self.inner.relocation_receipts.lock().await;
        let mut pruned = Vec::new();
        while index.len() > RECEIPT_LIMIT {
            if let Some(((_, move_id), _)) = index.pop_first() {
                pruned.push(move_id);
            }
        }
        drop(index);
        for move_id in pruned {
            if let Some(path) = self.relocation_record_path(move_id) {
                tokio::fs::remove_file(&path).await.map_err(|error| {
                    SyncError::Storage(format!(
                        "prune relocation receipt {}: {error}",
                        path.display()
                    ))
                })?;
            }
        }
        Ok(())
    }

    pub(super) async fn relocate_subtree_in_memory(
        &self,
        request: BlockRelocationRequest,
    ) -> SyncResult<BlockRelocationOutcome> {
        let source_lock = self.apply_lock_for_note(request.source_note_id).await;
        let destination_lock = if request.destination_note_id == request.source_note_id {
            None
        } else {
            Some(self.apply_lock_for_note(request.destination_note_id).await)
        };
        let source_first = request.source_note_id <= request.destination_note_id;

        if source_first {
            let _source_guard = source_lock.lock().await;
            let _destination_guard = match destination_lock.as_ref() {
                Some(lock) => Some(lock.lock().await),
                None => None,
            };
            let _ownership_guard = self.inner.ownership_transition.lock().await;
            self.relocate_under_locks(request).await
        } else {
            let destination_lock = destination_lock
                .as_ref()
                .expect("different note ids always have a destination lock");
            let _destination_guard = destination_lock.lock().await;
            let _source_guard = source_lock.lock().await;
            let _ownership_guard = self.inner.ownership_transition.lock().await;
            self.relocate_under_locks(request).await
        }
    }
}
