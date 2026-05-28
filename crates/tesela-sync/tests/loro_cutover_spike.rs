//! Phase 0 spike for the Loro cutover (.docs/ai/phases/2026-05-28-loro-cutover-spec.md).
//!
//! Throwaway validation of the load-bearing assumptions before any
//! production engine code. Delete after Phase 0 sign-off. Validates:
//!
//! 1. Two independent LoroDocs (= two devices) converge on concurrent
//!    edits via update export/import keyed by version vector — the
//!    "flashing"-fix proof.
//! 2. The exact flashing scenario: concurrent edits to the SAME block's
//!    text converge to one deterministic state on both sides (no
//!    ping-pong).
//! 3. Out-of-order / gapped update delivery still converges (relay is a
//!    store-and-forward log; delivery order isn't guaranteed).
//! 4. VersionVector encodes/decodes for transport — it's the per-doc
//!    relay cursor that replaces the HLC outbound cursor.
//! 5. A snapshot bootstraps a fresh doc (new-device join).

use loro::{ExportMode, LoroDoc, LoroValue, TreeParentId, VersionVector};

/// Insert a block (tree node under root) with text into a doc's "blocks"
/// tree. Returns nothing — mirrors how the real engine appends.
fn add_block(doc: &LoroDoc, text: &str) {
    let tree = doc.get_tree("blocks");
    let node = tree.create(TreeParentId::Root).unwrap();
    let meta = tree.get_meta(node).unwrap();
    meta.insert("text", text).unwrap();
    doc.commit();
}

/// Read all block texts in tree child order.
fn block_texts(doc: &LoroDoc) -> Vec<String> {
    let tree = doc.get_tree("blocks");
    let mut out = Vec::new();
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        let meta = tree.get_meta(node).unwrap();
        if let Some(v) = meta.get("text") {
            if let Ok(LoroValue::String(s)) = v.into_value() {
                out.push((*s).to_string());
            }
        }
    }
    out
}

/// Sync helper: ship updates from `src` that `dst` doesn't have yet,
/// using dst's version vector as the "since" cursor — exactly the relay
/// produce/apply contract.
fn sync_one_way(src: &LoroDoc, dst: &LoroDoc) {
    let dst_vv = dst.oplog_vv();
    let updates = src.export(ExportMode::updates(&dst_vv)).unwrap();
    if !updates.is_empty() {
        dst.import(&updates).unwrap();
    }
}

#[test]
fn two_docs_converge_on_disjoint_concurrent_edits() {
    let a = LoroDoc::new();
    let b = LoroDoc::new();

    // Both start from a shared base, then edit independently (offline).
    add_block(&a, "shared root");
    let base = a.export(ExportMode::all_updates()).unwrap();
    b.import(&base).unwrap();

    add_block(&a, "from A");
    add_block(&b, "from B");

    // Exchange updates both ways (two relay ticks).
    sync_one_way(&a, &b);
    sync_one_way(&b, &a);

    let ta = block_texts(&a);
    let tb = block_texts(&b);
    assert_eq!(ta, tb, "docs must converge to identical block order");
    assert!(ta.contains(&"from A".to_string()));
    assert!(ta.contains(&"from B".to_string()));
}

#[test]
fn concurrent_same_block_text_edit_converges_no_pingpong() {
    // The 2026-05-28 flashing scenario: both devices edit the SAME
    // block's text concurrently. The hand-rolled LWW engine ping-pongs.
    // Loro must converge to ONE deterministic value on BOTH sides.
    let a = LoroDoc::new();
    let tree_a = a.get_tree("blocks");
    let node = tree_a.create(TreeParentId::Root).unwrap();
    tree_a.get_meta(node).unwrap().insert("text", "original").unwrap();
    a.commit();

    let b = LoroDoc::new();
    b.import(&a.export(ExportMode::all_updates()).unwrap()).unwrap();

    // Concurrent conflicting edits to the same node's "text".
    {
        let t = a.get_tree("blocks");
        let n = t.children(TreeParentId::Root).unwrap()[0];
        t.get_meta(n).unwrap().insert("text", "A says posts were it can").unwrap();
        a.commit();
    }
    {
        let t = b.get_tree("blocks");
        let n = t.children(TreeParentId::Root).unwrap()[0];
        t.get_meta(n).unwrap().insert("text", "B says send out").unwrap();
        b.commit();
    }

    // Exchange and re-exchange (simulate the relay ping-pong window).
    for _ in 0..3 {
        sync_one_way(&a, &b);
        sync_one_way(&b, &a);
    }

    let ta = block_texts(&a);
    let tb = block_texts(&b);
    assert_eq!(ta, tb, "both sides must agree (no flashing)");
    assert_eq!(ta.len(), 1, "still one block");
    // The winner is deterministic — re-running sync doesn't change it.
    sync_one_way(&a, &b);
    sync_one_way(&b, &a);
    assert_eq!(block_texts(&a), ta, "stable — no further oscillation");
    assert_eq!(block_texts(&b), tb, "stable — no further oscillation");
}

#[test]
fn out_of_order_and_gapped_updates_converge() {
    // Relay is store-and-forward; a device may receive update 3 before
    // update 2. Loro buffers pending updates and converges once the gap
    // fills.
    let src = LoroDoc::new();
    add_block(&src, "u1");
    let u1 = src.export(ExportMode::all_updates()).unwrap();
    let vv1 = src.oplog_vv();

    add_block(&src, "u2");
    let u2 = src.export(ExportMode::updates(&vv1)).unwrap();
    let vv2 = src.oplog_vv();

    add_block(&src, "u3");
    let u3 = src.export(ExportMode::updates(&vv2)).unwrap();

    // Deliver to dst OUT OF ORDER: u1, then u3 (gap), then u2.
    let dst = LoroDoc::new();
    dst.import(&u1).unwrap();
    dst.import(&u3).unwrap(); // gap — u2 missing; Loro should buffer.
    dst.import(&u2).unwrap(); // fills the gap.

    assert_eq!(
        block_texts(&dst),
        vec!["u1", "u2", "u3"],
        "converges to full state regardless of delivery order"
    );
}

#[test]
fn version_vector_round_trips_for_transport() {
    // The per-doc VV is the relay cursor (replaces HLC outbound cursor).
    // It must encode → bytes → decode to drive ExportMode::updates.
    let doc = LoroDoc::new();
    add_block(&doc, "x");
    let vv = doc.oplog_vv();
    let bytes = vv.encode();
    let decoded = VersionVector::decode(&bytes).unwrap();
    assert_eq!(vv, decoded, "VV must survive encode/decode for the wire");

    // And a peer can use the decoded VV to compute the right delta.
    add_block(&doc, "y");
    let delta = doc.export(ExportMode::updates(&decoded)).unwrap();
    let fresh = LoroDoc::new();
    fresh.import(&doc.export(ExportMode::updates(&VersionVector::new())).unwrap())
        .ok();
    // Sanity: delta is non-empty (there were ops after the VV).
    assert!(!delta.is_empty(), "delta since old VV should carry the new op");
}

/// Phase 1 schema prototype: a per-note doc that round-trips FULL note
/// content byte-identically — frontmatter (verbatim) + an ordered list
/// of segments, each either a `bullet` (indent + text → `- text`) or a
/// `raw` line (verbatim, e.g. page-property `query::` lines, `# header`,
/// blank lines). This is the representation Phase 1 will use to drive
/// the ~13 non-bullet structural divergences to zero.
mod full_content {
    use super::*;

    /// Build a note doc from a frontmatter string + segments, then
    /// serialize it back. Each segment is (is_bullet, indent, text).
    fn serialize(frontmatter: &str, segments: &[(bool, u16, &str)]) -> String {
        let doc = LoroDoc::new();
        let fm = doc.get_text("frontmatter");
        fm.insert(0, frontmatter).unwrap();
        let body = doc.get_tree("body");
        for (is_bullet, indent, text) in segments {
            let n = body.create(TreeParentId::Root).unwrap();
            let m = body.get_meta(n).unwrap();
            m.insert("kind", if *is_bullet { "bullet" } else { "raw" }).unwrap();
            m.insert("indent", *indent as i64).unwrap();
            m.insert("text", *text).unwrap();
        }
        doc.commit();
        render(&doc)
    }

    /// Render a note doc to markdown — mirrors what the real engine will
    /// materialize to disk.
    fn render(doc: &LoroDoc) -> String {
        let mut out = String::new();
        let fm = doc.get_text("frontmatter").to_string();
        if !fm.is_empty() {
            out.push_str(&fm);
        }
        let body = doc.get_tree("body");
        for n in body.children(TreeParentId::Root).unwrap_or_default() {
            let m = body.get_meta(n).unwrap();
            let kind = m
                .get("kind")
                .and_then(|v| v.into_value().ok())
                .and_then(|v| if let LoroValue::String(s) = v { Some((*s).to_string()) } else { None })
                .unwrap_or_default();
            let indent = m
                .get("indent")
                .and_then(|v| v.into_value().ok())
                .and_then(|v| if let LoroValue::I64(i) = v { Some(i) } else { None })
                .unwrap_or(0) as usize;
            let text = m
                .get("text")
                .and_then(|v| v.into_value().ok())
                .and_then(|v| if let LoroValue::String(s) = v { Some((*s).to_string()) } else { None })
                .unwrap_or_default();
            if kind == "bullet" {
                out.push_str(&"  ".repeat(indent));
                out.push_str("- ");
                out.push_str(&text);
                out.push('\n');
            } else {
                out.push_str(&text);
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn page_property_note_round_trips() {
        // A query/page-property note — NO bullets. This is the class
        // that renders empty in today's shadow (parse_note ignores
        // non-bullet lines).
        let fm = "---\ntitle: Saved\n---\n\n";
        let segs = [
            (false, 0, "query:: kind:page"),
            (false, 0, "sort:: modified desc"),
            (false, 0, "icon:: clock"),
        ];
        let out = serialize(fm, &segs);
        assert_eq!(
            out,
            "---\ntitle: Saved\n---\n\nquery:: kind:page\nsort:: modified desc\nicon:: clock\n"
        );
    }

    #[test]
    fn mixed_bullets_and_raw_round_trips() {
        let fm = "---\ntitle: Mixed\n---\n\n";
        let segs = [
            (false, 0, "# 2026-05-17"),
            (true, 0, "a top bullet"),
            (true, 1, "a nested bullet"),
            (false, 0, "type:: ChatGPT"),
        ];
        let out = serialize(fm, &segs);
        assert_eq!(
            out,
            "---\ntitle: Mixed\n---\n\n# 2026-05-17\n- a top bullet\n  - a nested bullet\ntype:: ChatGPT\n"
        );
    }

    #[test]
    fn full_content_doc_converges_across_devices() {
        // Two devices edit different segments of a page-property note;
        // converge with no loss.
        let a = LoroDoc::new();
        a.get_text("frontmatter").insert(0, "---\ntitle: T\n---\n\n").unwrap();
        let body = a.get_tree("body");
        let n = body.create(TreeParentId::Root).unwrap();
        body.get_meta(n).unwrap().insert("kind", "raw").unwrap();
        body.get_meta(n).unwrap().insert("text", "query:: kind:page").unwrap();
        a.commit();

        let b = LoroDoc::new();
        b.import(&a.export(ExportMode::all_updates()).unwrap()).unwrap();

        // A appends a raw line; B edits the frontmatter — disjoint.
        {
            let t = a.get_tree("body");
            let n2 = t.create(TreeParentId::Root).unwrap();
            t.get_meta(n2).unwrap().insert("kind", "raw").unwrap();
            t.get_meta(n2).unwrap().insert("text", "sort:: modified").unwrap();
            a.commit();
        }
        {
            let fm = b.get_text("frontmatter");
            fm.delete(0, fm.len_unicode()).unwrap();
            fm.insert(0, "---\ntitle: T2\n---\n\n").unwrap();
            b.commit();
        }
        sync_one_way(&a, &b);
        sync_one_way(&b, &a);
        assert_eq!(render(&a), render(&b), "full-content note converges");
        assert!(render(&a).contains("sort:: modified"));
        assert!(render(&a).contains("title: T2"));
    }
}

#[test]
fn snapshot_bootstraps_a_fresh_device() {
    // New device join (Savanne): bootstrap from a snapshot, then keep
    // up via incremental updates.
    let origin = LoroDoc::new();
    for t in ["a", "b", "c"] {
        add_block(&origin, t);
    }
    let snapshot = origin.export(ExportMode::Snapshot).unwrap();

    let joiner = LoroDoc::new();
    joiner.import(&snapshot).unwrap();
    assert_eq!(block_texts(&joiner), vec!["a", "b", "c"]);

    // Incremental update after bootstrap.
    let vv = joiner.oplog_vv();
    add_block(&origin, "d");
    joiner
        .import(&origin.export(ExportMode::updates(&vv)).unwrap())
        .unwrap();
    assert_eq!(block_texts(&joiner), vec!["a", "b", "c", "d"]);
}
