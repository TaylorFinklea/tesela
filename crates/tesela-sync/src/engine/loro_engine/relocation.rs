use super::*;

#[derive(Clone)]
struct RelocatedBlockSnapshot {
    source_node: TreeID,
    bid: [u8; 16],
    text: String,
    indent: u16,
    parent: Option<[u8; 16]>,
    props: Vec<(String, ResolvedValue)>,
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
            .map(|(props, prop_keys)| prop_containers::read_props_typed(&props, &prop_keys))
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
    let (props, prop_keys) = prop_containers::node_prop_containers(&meta)?;
    for (key, value) in &block.props {
        match value {
            ResolvedValue::Scalar(value) => {
                prop_containers::prop_set_scalar(&props, &prop_keys, key, value)?;
            }
            ResolvedValue::Text(value) => {
                prop_containers::prop_set_text(&props, &prop_keys, key, value)?;
            }
            ResolvedValue::List(values) => {
                let _ = prop_containers::prop_ensure_list(&props, &prop_keys, key)?;
                for value in values {
                    prop_containers::prop_add_to_list(&props, &prop_keys, key, value)?;
                }
            }
        }
    }
    Ok(())
}

impl LoroEngine {
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

        let old_root_indent = blocks[0].indent;
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

    fn apply_same_note_relocation(&self, prepared: &PreparedRelocation) -> SyncResult<()> {
        let tree = prepared.source_doc.get_tree("blocks");
        if prepared.insertion_index < prepared.destination_order_without_source.len() {
            let anchor = prepared.destination_order_without_source[prepared.insertion_index];
            for block in &prepared.blocks {
                tree.mov_before(block.source_node, anchor)
                    .map_err(|error| {
                        SyncError::Storage(format!("loro relocation move before: {error}"))
                    })?;
            }
        } else if let Some(mut anchor) = prepared.destination_order_without_source.last().copied() {
            for block in &prepared.blocks {
                tree.mov_after(block.source_node, anchor).map_err(|error| {
                    SyncError::Storage(format!("loro relocation move after: {error}"))
                })?;
                anchor = block.source_node;
            }
        }
        for (index, block) in prepared.blocks.iter().enumerate() {
            apply_snapshot_metadata(
                &tree,
                block.source_node,
                block,
                prepared.new_indents[index],
                prepared.new_parents[index],
            )?;
        }
        prepared.source_doc.commit();
        Ok(())
    }

    async fn apply_cross_note_relocation(
        &self,
        prepared: &PreparedRelocation,
    ) -> SyncResult<LoroDoc> {
        let destination_doc = match prepared.destination_doc.as_ref() {
            Some(doc) => doc.clone(),
            None => self.seeded_destination_doc(&prepared.request)?,
        };
        let destination_tree = destination_doc.get_tree("blocks");
        for (offset, block) in prepared.blocks.iter().enumerate() {
            let node = destination_tree
                .create_at(TreeParentId::Root, prepared.insertion_index + offset)
                .map_err(|error| {
                    SyncError::Storage(format!("loro relocation destination create: {error}"))
                })?;
            author_snapshot_block(
                &destination_tree,
                node,
                block,
                prepared.new_indents[offset],
                prepared.new_parents[offset],
            )?;
        }
        destination_doc.commit();
        if prepared.destination_created {
            self.inner.docs.write().await.insert(
                prepared.request.destination_note_id,
                destination_doc.clone(),
            );
        }

        let source_tree = prepared.source_doc.get_tree("blocks");
        for block in &prepared.blocks {
            source_tree.delete(block.source_node).map_err(|error| {
                SyncError::Storage(format!("loro relocation source delete: {error}"))
            })?;
        }
        prepared.source_doc.commit();
        Ok(destination_doc)
    }

    async fn apply_prepared_relocation_under_locks(
        &self,
        prepared: PreparedRelocation,
    ) -> SyncResult<BlockRelocationOutcome> {
        if prepared.no_op {
            return Ok(BlockRelocationOutcome {
                move_id: prepared.request.move_id,
                status: BlockRelocationStatus::NoOp,
                notes: vec![RelocatedNoteVersion {
                    note_id: prepared.request.source_note_id,
                    slug: prepared.request.source_slug,
                    pre_version: prepared.source_pre_version,
                    changed: false,
                    created: false,
                }],
            });
        }

        let same_note = prepared.request.source_note_id == prepared.request.destination_note_id;
        if same_note {
            self.apply_same_note_relocation(&prepared)?;
            self.refresh_note_derived_under_ownership(
                prepared.request.source_note_id,
                &prepared.source_doc,
            )
            .await;
            return Ok(BlockRelocationOutcome {
                move_id: prepared.request.move_id,
                status: BlockRelocationStatus::Applied,
                notes: vec![RelocatedNoteVersion {
                    note_id: prepared.request.source_note_id,
                    slug: prepared.request.source_slug,
                    pre_version: prepared.source_pre_version,
                    changed: true,
                    created: false,
                }],
            });
        }

        let destination_doc = self.apply_cross_note_relocation(&prepared).await?;
        self.refresh_note_derived_under_ownership(
            prepared.request.source_note_id,
            &prepared.source_doc,
        )
        .await;
        self.refresh_note_derived_under_ownership(
            prepared.request.destination_note_id,
            &destination_doc,
        )
        .await;
        Ok(BlockRelocationOutcome {
            move_id: prepared.request.move_id,
            status: BlockRelocationStatus::Applied,
            notes: vec![
                RelocatedNoteVersion {
                    note_id: prepared.request.source_note_id,
                    slug: prepared.request.source_slug,
                    pre_version: prepared.source_pre_version,
                    changed: true,
                    created: false,
                },
                RelocatedNoteVersion {
                    note_id: prepared.request.destination_note_id,
                    slug: prepared.request.destination_slug,
                    pre_version: prepared.destination_pre_version,
                    changed: true,
                    created: prepared.destination_created,
                },
            ],
        })
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
            let prepared = self.prepare_relocation_under_locks(request).await?;
            self.apply_prepared_relocation_under_locks(prepared).await
        } else {
            let destination_lock = destination_lock
                .as_ref()
                .expect("different note ids always have a destination lock");
            let _destination_guard = destination_lock.lock().await;
            let _source_guard = source_lock.lock().await;
            let _ownership_guard = self.inner.ownership_transition.lock().await;
            let prepared = self.prepare_relocation_under_locks(request).await?;
            self.apply_prepared_relocation_under_locks(prepared).await
        }
    }
}
