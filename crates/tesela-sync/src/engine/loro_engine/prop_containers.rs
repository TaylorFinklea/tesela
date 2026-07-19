// ---------------------------------------------------------------------------
// Typed property containers (Phase-1 foundation).
//
// Each owner (a block node's meta map, or the doc root for page properties)
// carries a `props` LoroMap keyed by property name plus a sibling `prop_keys`
// LoroList recording first-seen key order — LoroMap iteration order is NOT
// guaranteed, so the materializer/index walk `prop_keys`, never the map.
// Scalars are stored as primitive LoroValues (zero sub-containers); free text
// as a nested LoroText; multi-value as a deterministic mergeable LoroList
// (concurrent first-touch and later add/remove operations merge on the same
// container id). Text keeps the regular-child discipline used by
// `write_block_text`; list writes migrate an older regular child on first add.
// ---------------------------------------------------------------------------

use super::*;
use tesela_core::property::PropScalar;

/// A property value resolved from a Loro container without flattening its type.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ResolvedValue {
    /// Primitive scalar register.
    Scalar(PropScalar),
    /// Mergeable text container.
    Text(String),
    /// Ordered, stable-deduplicated list members.
    List(Vec<PropScalar>),
}

/// Get-or-create the `props` + `prop_keys` containers on a block node's meta map.
pub(super) fn node_prop_containers(
    meta: &loro::LoroMap,
) -> SyncResult<(loro::LoroMap, loro::LoroList)> {
    let props = meta
        .get_or_create_container("props", loro::LoroMap::new())
        .map_err(|e| SyncError::Storage(format!("loro props get_or_create: {e}")))?;
    let prop_keys = meta
        .get_or_create_container("prop_keys", loro::LoroList::new())
        .map_err(|e| SyncError::Storage(format!("loro prop_keys get_or_create: {e}")))?;
    Ok((props, prop_keys))
}

/// The page-level `props` + `prop_keys` containers live at the doc root.
pub(super) fn page_prop_containers(doc: &LoroDoc) -> (loro::LoroMap, loro::LoroList) {
    (doc.get_map("props"), doc.get_list("prop_keys"))
}

/// NON-MUTATING read of a block node's `props` + `prop_keys` containers,
/// returning `None` when either has never been created. The materializer
/// reads through here — `node_prop_containers` would `get_or_create` the
/// nested containers and dirty the doc on a pure render (same discipline as
/// `read_block_text` inspecting `text_seq` via `meta.get`, never minting).
pub(super) fn read_node_prop_containers(
    meta: &loro::LoroMap,
) -> Option<(loro::LoroMap, loro::LoroList)> {
    let props = meta.get("props")?.into_container().ok()?.into_map().ok()?;
    let prop_keys = meta
        .get("prop_keys")?
        .into_container()
        .ok()?
        .into_list()
        .ok()?;
    Some((props, prop_keys))
}

/// The keys in `prop_keys` insertion order (raw — the dedup / drop-missing
/// reconcile shared by the materializer and index lands in P1.3).
fn prop_keys_ordered(prop_keys: &loro::LoroList) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..prop_keys.len() {
        if let Some(v) = prop_keys.get(i) {
            if let Ok(val) = v.into_value() {
                if let Ok(s) = val.into_string() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// All keys present in the `props` map (primitive values AND nested
/// containers), in the map's (unspecified) iteration order.
fn props_map_keys(props: &loro::LoroMap) -> Vec<String> {
    props.keys().map(|k| k.to_string()).collect()
}

/// The canonical ordered key set shared by the materializer + index + chips.
///
/// Walk `prop_keys` keeping the FIRST occurrence of each key and DROPPING any
/// key absent from `props`; then APPEND any key present in `props` but missing
/// from `prop_keys`, in lexicographic (byte) order. `prop_keys` is the sole
/// ordering authority — the `props` map's iteration order is never trusted for
/// order, only consulted for membership and for the lexicographic tail.
fn prop_keys_resolved(props: &loro::LoroMap, prop_keys: &loro::LoroList) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for key in prop_keys_ordered(prop_keys) {
        if seen.contains(&key) {
            continue;
        }
        if props.get(&key).is_none() {
            continue;
        }
        seen.insert(key.clone());
        out.push(key);
    }
    let mut tail: Vec<String> = props_map_keys(props)
        .into_iter()
        .filter(|k| !seen.contains(k))
        .collect();
    tail.sort();
    out.extend(tail);
    out
}

/// Append `key` to `prop_keys` once (first-seen order); no-op if present.
fn prop_keys_ensure(prop_keys: &loro::LoroList, key: &str) -> SyncResult<()> {
    if prop_keys_ordered(prop_keys)
        .iter()
        .any(|k| k.as_str() == key)
    {
        return Ok(());
    }
    prop_keys
        .push(key)
        .map_err(|e| SyncError::Storage(format!("loro prop_keys push: {e}")))
}

/// Remove every occurrence of `key` from `prop_keys`.
fn prop_keys_remove(prop_keys: &loro::LoroList, key: &str) -> SyncResult<()> {
    let mut i = 0;
    while i < prop_keys.len() {
        let matches = prop_keys
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(|val| val.into_string().ok())
            .map(|s| s.as_str() == key)
            .unwrap_or(false);
        if matches {
            prop_keys
                .delete(i, 1)
                .map_err(|e| SyncError::Storage(format!("loro prop_keys delete: {e}")))?;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Convert a primitive LoroValue back into a `PropScalar` (containers → None).
fn loro_value_to_scalar(v: loro::LoroValue) -> Option<PropScalar> {
    match v {
        loro::LoroValue::String(s) => Some(PropScalar::Text(s.to_string())),
        loro::LoroValue::I64(i) => Some(PropScalar::Int(i)),
        loro::LoroValue::Double(f) => Some(PropScalar::Float(f)),
        loro::LoroValue::Bool(b) => Some(PropScalar::Bool(b)),
        _ => None,
    }
}

fn map_insert_scalar(props: &loro::LoroMap, key: &str, value: &PropScalar) -> SyncResult<()> {
    let r = match value {
        PropScalar::Text(s) => props.insert(key, s.as_str()),
        PropScalar::Int(i) => props.insert(key, *i),
        PropScalar::Float(f) => props.insert(key, *f),
        PropScalar::Bool(b) => props.insert(key, *b),
    };
    r.map_err(|e| SyncError::Storage(format!("loro props insert: {e}")))
}

/// Property-representation-collision guard (tesela-ows.1 step 2, round 3).
///
/// A `props` map key can hold EITHER a primitive scalar (`SetScalar`) OR a
/// nested child container — a `LoroText` (free text) or a `LoroList`
/// (multi-value). The three lifecycle writers (the engine hook + the two
/// HTTP roll persisters) historically authored scalars, while the route's
/// free-text write authors a `LoroText`. When those representations collide
/// at one key, `get_or_create_container` HARD-ERRORS ("Expected value type
/// Text but found Value(String(..))") — surfacing as an HTTP 500 that turns
/// a recurring completion into a one-shot.
///
/// The invariant this restores: WRITING A PROPERTY VALUE MUST NEVER FAIL
/// BECAUSE OF THE PRIOR REPRESENTATION OF THAT KEY. If `key` currently holds
/// an occupant that is NOT a child container of `want`'s kind (a scalar, or
/// a container of a different kind), delete it from `props` so the caller's
/// `get_or_create_container` mints a fresh child of the right kind. A
/// same-kind child, an absent key, or a `Null` are left untouched — the
/// caller adopts / creates them (`get_or_create_container` treats `Null` as
/// absent). Only `props` is touched; the key stays in `prop_keys`, so
/// first-seen render order is preserved across a representation switch.
///
/// `SetText` keeps the regular-op-id-child discipline because
/// `ensure_mergeable_text` rejects the regular text children already shipped
/// in the fleet. Lists have an explicit migrate-and-replay path in
/// [`prop_ensure_list`], where a deterministic container id is required for
/// concurrent first-touch union semantics.
///
/// Concurrent convergence: two peers each clearing a scalar and creating a
/// regular child at the same key produce two op-id children; map conflict
/// resolution deterministically picks one (both peers agree) — it CONVERGES
/// without error, matching the existing `write_block_text`/prop discipline.
fn clear_incompatible_child(
    props: &loro::LoroMap,
    key: &str,
    want: loro::ContainerType,
) -> SyncResult<()> {
    let compatible = match props.get(key) {
        None => true,
        Some(loro::ValueOrContainer::Container(c)) => c.get_type() == want,
        Some(loro::ValueOrContainer::Value(loro::LoroValue::Null)) => true,
        Some(loro::ValueOrContainer::Value(_)) => false,
    };
    if !compatible {
        props
            .delete(key)
            .map_err(|e| SyncError::Storage(format!("loro props clear {key}: {e}")))?;
    }
    Ok(())
}

/// Set a single-value scalar property (primitive LoroValue; concurrent set = LWW).
///
/// Representation-tolerant by construction: a `LoroMap` insert of a
/// primitive REPLACES whatever child (scalar or container) the key held, so
/// a scalar write over a prior `LoroText`/`LoroList` needs no explicit clear.
pub(super) fn prop_set_scalar(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    map_insert_scalar(props, key, value)?;
    prop_keys_ensure(prop_keys, key)
}

/// Set a free-text property as a nested LoroText (concurrent char-merge).
/// Tolerates any prior representation at `key` (scalar or a different
/// container kind) via [`clear_incompatible_child`].
pub(super) fn prop_set_text(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    text: &str,
) -> SyncResult<()> {
    clear_incompatible_child(props, key, loro::ContainerType::Text)?;
    let t: LoroText = props
        .get_or_create_container(key, LoroText::new())
        .map_err(|e| SyncError::Storage(format!("loro prop text get_or_create: {e}")))?;
    t.update(text, UpdateOptions::default())
        .map_err(|e| SyncError::Storage(format!("loro prop text update: {e}")))?;
    prop_keys_ensure(prop_keys, key)
}

fn list_push_scalar(list: &loro::LoroList, value: &PropScalar) -> SyncResult<()> {
    let r = match value {
        PropScalar::Text(s) => list.push(s.as_str()),
        PropScalar::Int(i) => list.push(*i),
        PropScalar::Float(f) => list.push(*f),
        PropScalar::Bool(b) => list.push(*b),
    };
    r.map_err(|e| SyncError::Storage(format!("loro prop list push: {e}")))
}

/// Index of `value` in a list, or None.
fn list_position(list: &loro::LoroList, value: &PropScalar) -> Option<usize> {
    for i in 0..list.len() {
        if let Some(v) = list.get(i) {
            if let Ok(val) = v.into_value() {
                if loro_value_to_scalar(val).as_ref() == Some(value) {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn get_list_container(props: &loro::LoroMap, key: &str) -> Option<loro::LoroList> {
    props
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_list().ok())
}

/// Ensure a multi-value property has an ordered-list container and key even
/// when it currently has no members.
pub(super) fn prop_ensure_list(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
) -> SyncResult<loro::LoroList> {
    let list = match props.ensure_mergeable_list(key) {
        Ok(list) => list,
        Err(loro::LoroError::ArgErr(_)) => {
            // Fleet snapshots may hold a regular op-id LoroList (or a scalar /
            // text representation) at this key. Preserve list members, remove
            // that non-mergeable occupant, then replay them into the
            // deterministic mergeable child.
            let existing = get_list_container(props, key)
                .map(|list| {
                    (0..list.len())
                        .filter_map(|i| list.get(i))
                        .filter_map(|value| value.into_value().ok())
                        .filter_map(loro_value_to_scalar)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            props
                .delete(key)
                .map_err(|e| SyncError::Storage(format!("loro props clear {key}: {e}")))?;
            let list = props
                .ensure_mergeable_list(key)
                .map_err(|e| SyncError::Storage(format!("loro prop list ensure: {e}")))?;
            for value in existing {
                if list_position(&list, &value).is_none() {
                    list_push_scalar(&list, &value)?;
                }
            }
            list
        }
        Err(e) => return Err(SyncError::Storage(format!("loro prop list ensure: {e}"))),
    };
    prop_keys_ensure(prop_keys, key)?;
    Ok(list)
}

/// Add a value to a multi-value property's nested LoroList, union semantics
/// (a value already present is a no-op).
pub(super) fn prop_add_to_list(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    let list = prop_ensure_list(props, prop_keys, key)?;
    if list_position(&list, value).is_none() {
        list_push_scalar(&list, value)?;
    }
    Ok(())
}

/// Remove a value from a multi-value property's list (no-op if absent or the
/// property isn't a list). `prop_keys` is left intact — an emptied list keeps
/// its key until an explicit `prop_clear`.
pub(super) fn prop_remove_from_list(
    props: &loro::LoroMap,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    let Some(list) = get_list_container(props, key) else {
        return Ok(());
    };
    if let Some(i) = list_position(&list, value) {
        list.delete(i, 1)
            .map_err(|e| SyncError::Storage(format!("loro prop list delete: {e}")))?;
    }
    Ok(())
}

/// Remove a property entirely: from `props` AND `prop_keys`.
pub(super) fn prop_clear(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
) -> SyncResult<()> {
    props
        .delete(key)
        .map_err(|e| SyncError::Storage(format!("loro props delete: {e}")))?;
    prop_keys_remove(prop_keys, key)
}

/// Read a scalar property (primitive value under `key`); None if absent or a container.
#[allow(dead_code)] // scalar reader wired by the materializer in P1.5; tests read it now
pub(super) fn prop_get_scalar(props: &loro::LoroMap, key: &str) -> Option<PropScalar> {
    let val = props.get(key)?.into_value().ok()?;
    loro_value_to_scalar(val)
}

/// Read a text property (nested LoroText); None if absent or not text.
fn prop_get_text(props: &loro::LoroMap, key: &str) -> Option<String> {
    props
        .get(key)?
        .into_container()
        .ok()?
        .into_text()
        .ok()
        .map(|t| t.to_string())
}

/// Read a multi-value property as scalars in list order; empty if absent.
pub(super) fn prop_get_list(props: &loro::LoroMap, key: &str) -> Vec<PropScalar> {
    let Some(list) = get_list_container(props, key) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..list.len() {
        if let Some(s) = list
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(loro_value_to_scalar)
        {
            out.push(s);
        }
    }
    out
}

/// Read a multi-value property with STABLE first-occurrence dedup over the
/// merged list order. A concurrent union-merge across replicas can leave the
/// same value at more than one position; the materializer/index/chips read
/// through here so the view is deterministic (same CRDT state → same bytes).
fn prop_get_list_dedup(props: &loro::LoroMap, key: &str) -> Vec<PropScalar> {
    let mut seen: Vec<PropScalar> = Vec::new();
    for v in prop_get_list(props, key) {
        if !seen.contains(&v) {
            seen.push(v);
        }
    }
    seen
}

/// Materialize a `(props, prop_keys)` owner into the ordered, canonical
/// `(key, value)` pairs the `note_tree` serializer renders as `key:: value`
/// continuation lines. The ONE shared read path for the materializer (block
/// and page). Walks `prop_keys_resolved` order, rendering each stored kind:
/// a scalar via `format_scalar`; a multi-value list via stable-dedup
/// comma-join (the `tags::` convention); free text via the nested LoroText
/// string. Deterministic: same CRDT state always yields the same bytes.
pub(super) fn materialize_props(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
) -> Vec<(String, String)> {
    use tesela_core::property::format_scalar;
    let mut out: Vec<(String, String)> = Vec::new();
    for key in prop_keys_resolved(props, prop_keys) {
        let value = if let Some(scalar) = prop_get_scalar(props, &key) {
            format_scalar(&scalar)
        } else if get_list_container(props, &key).is_some() {
            prop_get_list_dedup(props, &key)
                .iter()
                .map(format_scalar)
                .collect::<Vec<_>>()
                .join(", ")
        } else if let Some(text) = prop_get_text(props, &key) {
            text
        } else {
            // A key in `prop_keys` whose `props` entry is absent is already
            // dropped by `prop_keys_resolved`; an unreadable container is
            // skipped rather than rendered as an empty line.
            continue;
        };
        out.push((key, value));
    }
    out
}

/// Read a `(props, prop_keys)` owner into ordered TYPED resolved values — the
/// twin-heal analog of [`materialize_props`] (which flattens to strings). Walks
/// `prop_keys_resolved` order, classifying each stored kind the same way the
/// materializer does: scalar (primitive) → [`ResolvedValue::Scalar`]; nested
/// list → [`ResolvedValue::List`] (stable-deduped); nested text →
/// [`ResolvedValue::Text`]. Used by the disjoint-twin heal to capture each
/// twin's typed props in the fork BEFORE the tombstone drops the loser.
pub(super) fn read_props_typed(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
) -> Vec<(String, ResolvedValue)> {
    let mut out: Vec<(String, ResolvedValue)> = Vec::new();
    for key in prop_keys_resolved(props, prop_keys) {
        let value = if let Some(scalar) = prop_get_scalar(props, &key) {
            ResolvedValue::Scalar(scalar)
        } else if get_list_container(props, &key).is_some() {
            ResolvedValue::List(prop_get_list_dedup(props, &key))
        } else if let Some(text) = prop_get_text(props, &key) {
            ResolvedValue::Text(text)
        } else {
            continue;
        };
        out.push((key, value));
    }
    out
}

#[cfg(test)]
mod prop_helper_tests {
    use super::*;
    use tesela_core::property::PropScalar;

    #[test]
    fn prop_helpers_set_get_clear_on_block_node() {
        let doc = loro::LoroDoc::new();
        let tree = doc.get_tree("blocks");
        let node = tree.create(loro::TreeParentId::Root).unwrap();
        let meta = tree.get_meta(node).unwrap();
        let (props, prop_keys) = node_prop_containers(&meta).unwrap();

        // scalar (string)
        prop_set_scalar(
            &props,
            &prop_keys,
            "status",
            &PropScalar::Text("doing".into()),
        )
        .unwrap();
        assert_eq!(
            prop_get_scalar(&props, "status"),
            Some(PropScalar::Text("doing".into()))
        );

        // scalar (int)
        prop_set_scalar(&props, &prop_keys, "priority", &PropScalar::Int(3)).unwrap();
        assert_eq!(
            prop_get_scalar(&props, "priority"),
            Some(PropScalar::Int(3))
        );

        // text (nested LoroText)
        prop_set_text(&props, &prop_keys, "note", "hello world").unwrap();
        assert_eq!(
            prop_get_text(&props, "note").as_deref(),
            Some("hello world")
        );

        // multi-value: add → union (dup is a no-op), then remove
        prop_add_to_list(&props, &prop_keys, "tags", &PropScalar::Text("Task".into())).unwrap();
        prop_add_to_list(
            &props,
            &prop_keys,
            "tags",
            &PropScalar::Text("Urgent".into()),
        )
        .unwrap();
        prop_add_to_list(&props, &prop_keys, "tags", &PropScalar::Text("Task".into())).unwrap();
        assert_eq!(
            prop_get_list(&props, "tags"),
            vec![
                PropScalar::Text("Task".into()),
                PropScalar::Text("Urgent".into())
            ]
        );
        prop_remove_from_list(&props, "tags", &PropScalar::Text("Task".into())).unwrap();
        assert_eq!(
            prop_get_list(&props, "tags"),
            vec![PropScalar::Text("Urgent".into())]
        );

        // prop_keys preserves first-seen order
        assert_eq!(
            prop_keys_ordered(&prop_keys).join(","),
            "status,priority,note,tags"
        );

        // clear removes from BOTH props and prop_keys
        prop_clear(&props, &prop_keys, "priority").unwrap();
        assert_eq!(prop_get_scalar(&props, "priority"), None);
        assert_eq!(prop_keys_ordered(&prop_keys).join(","), "status,note,tags");
    }

    #[test]
    fn prop_helpers_on_page_root() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);
        prop_set_scalar(&props, &prop_keys, "type", &PropScalar::Text("Tag".into())).unwrap();
        assert_eq!(
            prop_get_scalar(&props, "type"),
            Some(PropScalar::Text("Tag".into()))
        );
        assert_eq!(prop_keys_ordered(&prop_keys).join(","), "type");
    }

    #[test]
    fn prop_keys_resolved_dedups_drops_missing_appends_lexicographically() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);

        // props gets three keys via the set helpers (which also push to prop_keys);
        // then we hand-build a messy prop_keys list to exercise the reconcile:
        //   - "status"  : present in props, listed
        //   - "ghost"   : NOT in props, listed (must be DROPPED)
        //   - "status"  : duplicate (must keep FIRST occurrence only)
        //   - "priority": present in props, listed
        // and "zeta" is present in props but absent from prop_keys (must be
        // APPENDED in lexicographic order relative to other props-only keys).
        prop_set_scalar(
            &props,
            &prop_keys,
            "status",
            &PropScalar::Text("doing".into()),
        )
        .unwrap();
        prop_set_scalar(&props, &prop_keys, "priority", &PropScalar::Int(3)).unwrap();
        // props-only keys (NOT pushed to prop_keys): inserted directly so they
        // exercise the lexicographic append. "alpha" sorts before "zeta".
        props.insert("zeta", "z").unwrap();
        props.insert("alpha", "a").unwrap();

        // Overwrite prop_keys with the messy ordering described above.
        prop_keys.delete(0, prop_keys.len()).unwrap();
        for k in ["status", "ghost", "status", "priority"] {
            prop_keys.push(k).unwrap();
        }

        // Expected: listed-and-present in first-seen order (status, priority,
        // dup status dropped, ghost dropped), then props-only keys appended in
        // byte order (alpha, zeta).
        assert_eq!(
            prop_keys_resolved(&props, &prop_keys),
            vec![
                "status".to_string(),
                "priority".to_string(),
                "alpha".to_string(),
                "zeta".to_string(),
            ]
        );
    }

    #[test]
    fn prop_get_list_dedup_is_stable_first_occurrence() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);

        // Simulate a post-merge list where a value appears at more than one
        // position: [A, B, A, C, B]. The stable dedup keeps the FIRST sighting
        // of each value, preserving merged order: [A, B, C].
        let list: loro::LoroList = props
            .get_or_create_container("tags", loro::LoroList::new())
            .unwrap();
        for v in ["A", "B", "A", "C", "B"] {
            list.push(v).unwrap();
        }
        prop_keys.push("tags").unwrap();

        assert_eq!(
            prop_get_list_dedup(&props, "tags"),
            vec![
                PropScalar::Text("A".into()),
                PropScalar::Text("B".into()),
                PropScalar::Text("C".into()),
            ]
        );
    }
}
