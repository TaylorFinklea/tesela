//! CHARACTERIZATION (tesela-y11): pin the EXACT current convergence behavior of
//! the relay-inbound apply (`import_doc_update`) for disjoint-lineage twins, so
//! the durable y11 fix targets a real gap rather than a phantom.
//!
//! Background: the 2026-06-29 concurrent-convergence work added a genuine-edit
//! discriminator (`peer_genuine_block_changes`) + force-heal to `import_doc_update`
//! / `apply_doc_update_status`. This suite measures whether disjoint twins now
//! CONVERGE (and in how many rounds), whether any genuine edit is LOST, and
//! whether text is ever GARBLED (concatenated).

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

fn note_id(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

async fn author_disjoint(e: &LoroEngine, note: [u8; 16], bid: &str, text: &str) {
    e.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("daily".into()),
        title: "Daily".into(),
        content: format!("- {text} <!-- bid:{bid} -->\n"),
        created_at_millis: 1,
    })
    .await
    .unwrap();
}

/// One relay round-trip: each engine imports the delta the OTHER is missing
/// (exported since the receiver's current vv, mirroring produce_relay_updates).
async fn round_trip(a: &LoroEngine, b: &LoroEngine, note: [u8; 16]) {
    let a_needs = a.doc_version(note).await;
    let b_needs = b.doc_version(note).await;
    let for_a = b.export_doc_update(note, a_needs.as_deref()).await.unwrap();
    let for_b = a.export_doc_update(note, b_needs.as_deref()).await.unwrap();
    a.import_doc_update(note, &for_a).await.unwrap();
    b.import_doc_update(note, &for_b).await.unwrap();
}

fn assert_no_garble(txt: &str, parts: &[&str]) {
    // Garble = BOTH disjoint contributions concatenated into one block line.
    let concatenated = parts.iter().filter(|p| txt.contains(**p)).count() > 1;
    assert!(
        !concatenated,
        "GARBLE: block text contains >1 disjoint contribution concatenated: {txt:?}"
    );
}

/// C1: symmetric two-device disjoint genuine edits must converge in ONE round
/// (the y11 fix — previously this transiently split-brained for a round).
#[tokio::test]
async fn c1_symmetric_disjoint_converges_in_one_round_no_garble() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01");
    let a = engine(0xAA);
    let b = engine(0xBB);
    author_disjoint(&a, note, bid, "alphaA").await;
    author_disjoint(&b, note, bid, "betaB").await;

    round_trip(&a, &b, note).await;
    let (a1, b1) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
    eprintln!("C1 round1: A={a1:?} B={b1:?}");
    assert_no_garble(&a1, &["alphaA", "betaB"]);
    assert_no_garble(&b1, &["alphaA", "betaB"]);
    assert_eq!(a1, b1, "symmetric disjoint conflict must converge in ONE round");
    assert!(
        a1.contains("alphaA") || a1.contains("betaB"),
        "a genuine edit must survive (not empty): {a1:?}"
    );

    // A second round must be a stable no-op (idempotent — no ping-pong).
    round_trip(&a, &b, note).await;
    let (a2, b2) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
    eprintln!("C1 round2: A={a2:?} B={b2:?}");
    assert_eq!(a1, a2, "converged value must be stable across a further round (no ping-pong)");
    assert_eq!(a2, b2, "still converged");
}

/// C8 (FLIPPED by tesela-fte — pure max-`TreeID`): the "re-shipped stale value"
/// wire incident. One device re-ships an OLD value the other lineage already
/// superseded. Under the PURE max-`TreeID` rule the stale-guard is dropped, so
/// the winner is ONLY the higher-`TreeID` (higher-peer) twin's text — NOT
/// necessarily the newer value. Both peer orderings converge in ONE round; the
/// survivor flips with the ordering:
///   - evolver-peer > stale-peer → evolver's "Awesome sweet" (the newer value) wins;
///   - stale-peer  > evolver-peer → the re-shipped stale "Awesome" wins.
/// (Pre-fte the stale-guard forced "Awesome sweet" in BOTH orderings; product-
/// approved 2026-07-01: higher-TreeID text wins over genuine-edit preference.)
/// Both bytes are < 0x80, so the peer-byte order equals the masked-PeerID order.
#[tokio::test]
async fn c8_stale_reship_resolves_to_max_treeid_twin_both_orderings() {
    for (evolver_peer, stale_peer) in [(0x5e, 0x11), (0x11, 0x5e)] {
        let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
        let note = note_id("2026-07-01-stale");
        // Evolver: authored "Awesome" then edited to "Awesome sweet".
        let evolver = engine(evolver_peer);
        author_disjoint(&evolver, note, bid, "Awesome").await;
        evolver
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x0a; 16],
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome sweet".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        // Stale device: authored ONLY the old "Awesome" on a disjoint lineage.
        let stale = engine(stale_peer);
        author_disjoint(&stale, note, bid, "Awesome").await;

        round_trip(&evolver, &stale, note).await;
        let (ev, st) = (
            evolver.render_note(note).await.unwrap(),
            stale.render_note(note).await.unwrap(),
        );
        eprintln!("C8 (evolver={evolver_peer:#x} stale={stale_peer:#x}): evolver={ev:?} stale={st:?}");
        assert_eq!(ev, st, "must converge in one round");
        // "sweet" appears ONLY in the evolver's newer "Awesome sweet"; it
        // survives IFF the evolver's twin is the higher-`TreeID` (higher-peer).
        let evolver_wins = evolver_peer > stale_peer;
        assert_eq!(
            ev.contains("sweet"),
            evolver_wins,
            "pure max-`TreeID`: the higher-peer twin's text wins, stale or not \
             (evolver={evolver_peer:#x} stale={stale_peer:#x}): {ev:?}"
        );
    }
}

/// C2: asymmetric stale — device authored blind, server evolved base→edit. The
/// device must adopt the server's genuine edit in ONE import (regression guard).
#[tokio::test]
async fn c2_asymmetric_stale_device_adopts_server_edit_one_round() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-06-03");
    let server = engine(0x5e);
    author_disjoint(&server, note, bid, "server base").await;
    let device = engine(0x11);
    author_disjoint(&device, note, bid, "device mock").await;
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "web edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let snap = server.export_doc_update(note, None).await.unwrap();
    device.import_doc_update(note, &snap).await.unwrap();
    let d = device.render_note(note).await.unwrap();
    eprintln!("C2: device={d:?}");
    assert!(d.contains("web edit"), "device must adopt server edit: {d:?}");
    assert!(!d.contains("device mock"), "device's stale twin must not survive: {d:?}");
}

/// C3: after convergence, a SUBSEQUENT concurrent edit to the same block must
/// merge (shared lineage), not re-fork into a new lossy twin.
#[tokio::test]
async fn c3_no_refork_after_convergence() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01");
    let a = engine(0xAA);
    let b = engine(0xBB);
    author_disjoint(&a, note, bid, "alphaA").await;
    author_disjoint(&b, note, bid, "betaB").await;
    round_trip(&a, &b, note).await;
    round_trip(&a, &b, note).await;
    let converged = a.render_note(note).await.unwrap();
    assert_eq!(converged, b.render_note(note).await.unwrap(), "precondition: converged");

    // Now both edit the SAME block concurrently, appending distinct markers.
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0x0a; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "shared A-edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockUpsert {
        block_id: [0x0a; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "shared B-edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    round_trip(&a, &b, note).await;
    round_trip(&a, &b, note).await;
    let (af, bf) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
    eprintln!("C3 final: A={af:?} B={bf:?}");
    assert_eq!(af, bf, "subsequent concurrent edits must re-converge");
}

/// C4: re-importing the SAME snapshot must be idempotent (no change, no loss).
#[tokio::test]
async fn c4_reimport_is_idempotent() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-06-03");
    let server = engine(0x5e);
    author_disjoint(&server, note, bid, "server base").await;
    let device = engine(0x11);
    author_disjoint(&device, note, bid, "device mock").await;
    let snap = server.export_doc_update(note, None).await.unwrap();
    device.import_doc_update(note, &snap).await.unwrap();
    let once = device.render_note(note).await.unwrap();
    device.import_doc_update(note, &snap).await.unwrap();
    let twice = device.render_note(note).await.unwrap();
    eprintln!("C4: once={once:?} twice={twice:?}");
    assert_eq!(once, twice, "re-import must be idempotent");
}

/// C6: a disjoint note with TWO blocks edited differently on each device. Each
/// bid must resolve independently and both sides converge (no cross-block loss).
#[tokio::test]
async fn c6_multi_block_disjoint_converges() {
    let bid1 = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let bid2 = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
    let note = note_id("2026-07-01-multi");
    let a = engine(0xAA);
    let b = engine(0xBB);
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("daily".into()),
        title: "Daily".into(),
        content: format!("- one A <!-- bid:{bid1} -->\n- two A <!-- bid:{bid2} -->\n"),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("daily".into()),
        title: "Daily".into(),
        content: format!("- one B <!-- bid:{bid1} -->\n- two B <!-- bid:{bid2} -->\n"),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    for _ in 0..3 {
        round_trip(&a, &b, note).await;
    }
    let (af, bf) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
    eprintln!("C6 final: A={af:?} B={bf:?}");
    assert_eq!(af, bf, "multi-block disjoint note must converge on both sides");
    assert_no_garble(&af, &["one A", "one B"]);
    assert_no_garble(&af, &["two A", "two B"]);
}

/// C7: delete-vs-edit — one device deletes the block while the other edits it.
/// Per the deleted-wins semantics (project_block_delete_semantics), an explicit
/// BlockDelete must win and both sides converge (the block is gone, not garbled
/// or resurrected as a twin).
#[tokio::test]
async fn c7_delete_vs_edit_converges_deleted_wins() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-del");
    // Shared base first (so the block exists on one lineage), then diverge.
    let a = engine(0xAA);
    author_disjoint(&a, note, bid, "shared base").await;
    // B bootstraps off A's snapshot → shared lineage.
    let b = engine(0xBB);
    let base = a.export_doc_update(note, None).await.unwrap();
    b.import_doc_update(note, &base).await.unwrap();

    // A edits the block; B deletes it — concurrently.
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0x0a; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "edited on A".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockDelete { block_id: [0x0a; 16] })
        .await
        .unwrap();

    for _ in 0..2 {
        round_trip(&a, &b, note).await;
    }
    let (af, bf) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
    eprintln!("C7 final: A={af:?} B={bf:?}");
    assert_eq!(af, bf, "delete-vs-edit must converge on both sides");
    assert!(!af.contains("edited on A"), "deleted-wins: the edit must not resurrect the block: {af:?}");
}

/// C10: after a disjoint conflict converges, MANY further sync rounds must be a
/// stable no-op — no oscillation, and crucially no text GROWTH (the round-2
/// `betaBbetaB` doubling regression the keep-winner design fixes).
#[tokio::test]
async fn c10_convergence_is_stable_no_pingpong_growth() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-stable");
    let a = engine(0xAA);
    let b = engine(0xBB);
    author_disjoint(&a, note, bid, "alphaA").await;
    author_disjoint(&b, note, bid, "betaB").await;
    round_trip(&a, &b, note).await;
    let converged = a.render_note(note).await.unwrap();
    assert_eq!(converged, b.render_note(note).await.unwrap(), "precondition: converged round 1");
    for r in 0..5 {
        round_trip(&a, &b, note).await;
        let (af, bf) = (a.render_note(note).await.unwrap(), b.render_note(note).await.unwrap());
        assert_eq!(af, converged, "round {r}: value must stay stable (no ping-pong/growth): {af:?}");
        assert_eq!(af, bf, "round {r}: still converged");
    }
}

/// C11 (FLIPPED by tesela-fte — pure max-`TreeID`): three-way with a STALE
/// re-ship — an evolver (v1→v2), a device that re-ships the superseded v1 ON
/// THE HIGHEST-peer lineage (0xF0), and a device with a genuine third value w.
/// Pure max-`TreeID` drops the stale-guard, so the highest-`TreeID` twin wins
/// UNCONDITIONALLY: the stale re-shipped v1 (peer 0xF0 > 0xC0 > 0xA0) now WINS.
/// All three still converge to ONE value with no garble — the invariant that
/// matters — but the survivor is the max-`TreeID` twin's text, not the newest.
/// (Pre-fte the stale-guard excluded 0xF0's v1 and "w" won; product-approved
/// 2026-07-01 that higher-TreeID text wins.) This is the exact case the
/// dropped stale-guard used to protect: the flip is intentional.
#[tokio::test]
async fn c11_three_way_resolves_to_max_treeid_twin() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-3stale");
    let evolver = engine(0xA0);
    author_disjoint(&evolver, note, bid, "v1").await;
    evolver
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x0a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "v2".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let stale = engine(0xF0); // HIGHEST peer — wins under pure max-`TreeID`
    author_disjoint(&stale, note, bid, "v1").await;
    let fresh = engine(0xC0);
    author_disjoint(&fresh, note, bid, "w").await;

    for _ in 0..5 {
        round_trip(&evolver, &stale, note).await;
        round_trip(&stale, &fresh, note).await;
        round_trip(&evolver, &fresh, note).await;
    }
    let (ev, st, fr) = (
        evolver.render_note(note).await.unwrap(),
        stale.render_note(note).await.unwrap(),
        fresh.render_note(note).await.unwrap(),
    );
    eprintln!("C11 final: evolver={ev:?} stale={st:?} fresh={fr:?}");
    assert_eq!(ev, st, "evolver and stale must converge");
    assert_eq!(st, fr, "stale and fresh must converge");
    assert_no_garble(&ev, &["v1", "v2", "w"]);
    // Pure max-`TreeID`: the highest-peer twin (0xF0, holding v1) survives —
    // the stale-guard that formerly excluded it is gone.
    assert!(
        ev.contains("v1") && !ev.contains("v2") && !ev.contains("w"),
        "pure max-`TreeID` keeps the highest-peer (0xF0) twin's value v1: {ev:?}"
    );
}

/// C12 (adversarial #1): INCREMENTAL split delivery. S resolves the twin while
/// D's twin still looks EMPTY (D typed after S got the create), while D resolves
/// with its twin non-empty. If the keep decision depends on per-replica-view
/// emptiness, the two replicas tombstone DIFFERENT survivors → both die → the
/// block VANISHES. Must instead converge with the block intact.
#[tokio::test]
async fn c12_incremental_empty_twin_no_total_loss() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-inc");
    let s = engine(0x5e);
    author_disjoint(&s, note, bid, "content").await;

    // D creates the SAME bid as an EMPTY block first (disjoint), exports just
    // that create, then types "hello" as a separate delta.
    let d = engine(0x7f); // high peer → D's TreeID > S's
    d.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("daily".into()),
        title: "Daily".into(),
        content: format!("- <!-- bid:{bid} -->\n"),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    let d_create = d.export_doc_update(note, None).await.unwrap();

    // S imports D's EMPTY create — resolves the twin with D's tip empty.
    s.import_doc_update(note, &d_create).await.unwrap();

    // D types "hello".
    let pre = d.doc_version(note).await;
    d.record_local(OpPayload::BlockUpsert {
        block_id: [0x0a; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "hello".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let d_text = d.export_doc_update(note, pre.as_deref()).await.unwrap();

    // D imports S's snapshot — resolves the twin with D's tip = "hello".
    let s_snap = s.export_doc_update(note, None).await.unwrap();
    d.import_doc_update(note, &s_snap).await.unwrap();
    // S receives D's late text delta.
    s.import_doc_update(note, &d_text).await.unwrap();

    for _ in 0..3 {
        round_trip(&s, &d, note).await;
    }
    let (sf, df) = (s.render_note(note).await.unwrap(), d.render_note(note).await.unwrap());
    eprintln!("C12 final: S={sf:?} D={df:?}");
    assert_eq!(sf, df, "must converge (no cross-tombstone divergence)");
    assert!(
        sf.contains("content") || sf.contains("hello"),
        "the block must NOT vanish (total loss): {sf:?}"
    );
    assert_no_garble(&sf, &["content", "hello"]);
}

/// C13 (adversarial #2): one replica resolves the twin via the WS path
/// (`apply_relay_updates`) while the other catches up via the AUTHORITATIVE
/// snapshot path (`import_authoritative_snapshot`). If the two paths pick
/// survivors on incompatible axes, their tombstones cross → block vanishes.
#[tokio::test]
async fn c13_ws_vs_authoritative_no_cross_tombstone() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-auth");
    let s = engine(0x11); // low peer
    let d = engine(0x7f); // high peer → D's TreeID > S's
    author_disjoint(&s, note, bid, "server text").await;
    author_disjoint(&d, note, bid, "device text").await;

    let d_snap = d.export_doc_update(note, None).await.unwrap();
    let s_snap = s.export_doc_update(note, None).await.unwrap();
    // S resolves via the WS relay path.
    let _ = s.apply_relay_updates(&[(note, d_snap)]).await;
    // D catches up via the AUTHORITATIVE snapshot path.
    d.import_authoritative_snapshot(note, &s_snap).await.unwrap();

    for _ in 0..3 {
        round_trip(&s, &d, note).await;
    }
    let (sf, df) = (s.render_note(note).await.unwrap(), d.render_note(note).await.unwrap());
    eprintln!("C13 final: S={sf:?} D={df:?}");
    assert_eq!(sf, df, "WS and authoritative paths must agree (no cross-tombstone)");
    assert!(
        sf.contains("server text") || sf.contains("device text"),
        "the block must NOT vanish (total loss): {sf:?}"
    );
    assert_no_garble(&sf, &["server text", "device text"]);
}

/// C14 (tesela-49d): the one-shot local repair API. After a disjoint twin is
/// collapsed by the normal relay apply, `scan_disjoint_twins` finds nothing and
/// `heal_disjoint_twins` is a safe idempotent no-op — documenting that residue
/// self-heals on sync and the repair is the offline/force path. (The collapse
/// logic itself — the max-`TreeID` `tombstone_duplicate_twins` + the
/// `twin_winners_for` prop union — is covered by C1–C13; no public API leaves a
/// persistent twin post-fix.)
#[tokio::test]
async fn c14_repair_scan_heal_noop_on_healed_mosaic() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01-repair");
    let a = engine(0xA1);
    author_disjoint(&a, note, bid, "A text").await;
    let b = engine(0xB1);
    author_disjoint(&b, note, bid, "B text").await;
    let a_snap = a.export_doc_update(note, None).await.unwrap();
    let _ = b.apply_relay_updates(&[(note, a_snap)]).await;

    assert!(
        b.scan_disjoint_twins().await.is_empty(),
        "a healed mosaic has no disjoint twins to report"
    );
    assert!(
        b.heal_disjoint_twins().await.is_empty(),
        "heal is a safe no-op on a healed mosaic"
    );
    let r = b.render_note(note).await.unwrap();
    assert!(r.contains("A text") || r.contains("B text"), "block intact: {r:?}");
    assert_no_garble(&r, &["A text", "B text"]);
}

/// C5: three disjoint devices, all-to-all exchange. Do all three converge?
#[tokio::test]
async fn c5_three_device_disjoint_converges() {
    let bid = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    let note = note_id("2026-07-01");
    let a = engine(0xA0);
    let b = engine(0xB0);
    let c = engine(0xC0);
    author_disjoint(&a, note, bid, "fromA").await;
    author_disjoint(&b, note, bid, "fromB").await;
    author_disjoint(&c, note, bid, "fromC").await;
    // Several all-to-all rounds.
    for _ in 0..4 {
        round_trip(&a, &b, note).await;
        round_trip(&b, &c, note).await;
        round_trip(&a, &c, note).await;
    }
    let (af, bf, cf) = (
        a.render_note(note).await.unwrap(),
        b.render_note(note).await.unwrap(),
        c.render_note(note).await.unwrap(),
    );
    eprintln!("C5 final: A={af:?} B={bf:?} C={cf:?}");
    assert_no_garble(&af, &["fromA", "fromB", "fromC"]);
    assert_eq!(af, bf, "A and B must converge");
    assert_eq!(bf, cf, "B and C must converge");
}
