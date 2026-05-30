use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tesela_core::{
    db::SqliteIndex,
    note::NoteId,
    storage::filesystem::FsNoteStore,
    traits::{note_store::NoteStore, search_index::SearchIndex},
};
use tokio::sync::broadcast;

use crate::state::{AppState, ConnId, WsDelta, WsEvent};

/// Bidirectional `/ws` handler (instant-multidevice Phase A).
///
/// Two frame directions multiplexed on one socket:
/// - **text** = JSON [`WsEvent`] from `ws_tx` (unchanged; the web client's
///   query-invalidation path).
/// - **binary** = `TLR2`-framed Loro delta from `ws_delta_tx`, fanned out
///   to every *other* socket (echo-suppressed by connection id — a delta
///   is never sent back to the socket it arrived on).
///
/// Inbound binary frames are decoded, applied to the server engine
/// (`apply_relay_updates`, idempotent + commutative), then for each touched
/// note we emit a `WsEvent::NoteUpdated` on `ws_tx` (so web invalidates;
/// spec finding #4) and re-publish the exact applied bytes on `ws_delta_tx`
/// tagged with this socket's origin id so they reach the *other* sockets.
/// The hub forwards the bytes it received — it does NOT re-`produce` a
/// delta and never touches the relay's broadcast cursor (spec finding #3).
pub async fn ws_handler(ws: WebSocketUpgrade, State(s): State<Arc<AppState>>) -> impl IntoResponse {
    let conn_id: ConnId = s.ws_conn_seq.fetch_add(1, Ordering::Relaxed);
    ws.on_upgrade(move |socket| handle_socket(socket, s, conn_id))
}

async fn handle_socket(socket: WebSocket, s: Arc<AppState>, conn_id: ConnId) {
    let (mut sink, mut stream) = socket.split();
    let mut ws_rx = s.ws_tx.subscribe();
    let mut delta_rx = s.ws_delta_tx.subscribe();

    // Send task: fans both broadcast channels onto this socket. Text for
    // WsEvents, binary for Loro deltas (skipping this socket's own deltas).
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                evt = ws_rx.recv() => match evt {
                    Ok(event) => match serde_json::to_string(&event) {
                        Ok(msg) => {
                            if sink.send(Message::Text(msg.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => tracing::warn!("Failed to serialize WsEvent: {}", e),
                    },
                    // Lagged: a slow consumer fell behind. Keep going — the
                    // next recv resyncs to the channel tail.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                delta = delta_rx.recv() => match delta {
                    Ok(WsDelta { origin, frame }) => {
                        // Echo-suppression: never send a delta back to the
                        // socket it arrived on.
                        if origin == Some(conn_id) {
                            continue;
                        }
                        if sink.send(Message::Binary(frame.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
            }
        }
    });

    // Recv task: inbound binary delta frames → decode → apply → notify +
    // re-broadcast. Text/ping/pong/close are handled inline.
    let recv_state = Arc::clone(&s);
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Binary(bytes) => {
                    apply_inbound_delta(
                        &*recv_state.sync_engine,
                        &recv_state.store,
                        &recv_state.index,
                        &recv_state.ws_tx,
                        &recv_state.ws_delta_tx,
                        &bytes,
                        Some(conn_id),
                    )
                    .await;
                }
                Message::Close(_) => break,
                // Text frames inbound are not part of the protocol (clients
                // send deltas as binary); ignore. Ping/Pong are handled by
                // axum's keep-alive.
                _ => {}
            }
        }
    });

    // When either half ends (socket closed / channel closed), abort the
    // other so the connection's tasks don't leak — the orphaned send task
    // would otherwise linger until its next failed `sink.send`.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}

/// Decode a `TLR2`-framed delta, apply it to the engine, and fan out the
/// result: a `WsEvent::NoteUpdated` per touched note (web invalidation) and
/// a re-broadcast of the exact applied bytes on `ws_delta_tx` (origin-tagged
/// for echo-suppression). `origin` is the connection id of the socket the
/// frame arrived on, or `None` for HTTP/relay-originated deltas that fan out
/// to everyone. Shared by the WS recv path and the relay tick fan-out — it
/// takes the `AppState` pieces individually so the relay loop (which holds
/// clones, not the assembled `Arc<AppState>`) can call it too.
#[allow(clippy::too_many_arguments)]
pub async fn apply_inbound_delta(
    engine: &dyn tesela_sync::SyncEngine,
    store: &FsNoteStore,
    index: &SqliteIndex,
    ws_tx: &broadcast::Sender<WsEvent>,
    ws_delta_tx: &broadcast::Sender<WsDelta>,
    bytes: &[u8],
    origin: Option<ConnId>,
) {
    let updates = match tesela_sync::decode_loro_relay_payload(bytes) {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::debug!("ws: skip non-v2 binary frame ({} bytes)", bytes.len());
            return;
        }
        Err(e) => {
            tracing::warn!("ws: loro delta decode failed: {} (skipping)", e);
            return;
        }
    };
    let pairs: Vec<([u8; 16], Vec<u8>)> = updates
        .into_iter()
        .map(|u| (u.doc, u.update_bytes))
        .collect();
    let applied = engine.apply_relay_updates(&pairs).await;
    if applied == 0 {
        return;
    }
    // Notify web (finding #4) for each distinct touched note.
    let mut seen: Vec<[u8; 16]> = Vec::new();
    for (doc, _) in &pairs {
        if seen.contains(doc) {
            continue;
        }
        seen.push(*doc);
        emit_note_updated(engine, store, index, ws_tx, *doc).await;
    }
    // Re-broadcast the exact applied bytes to the OTHER sockets. The hub
    // forwards what it received — it does not re-`produce` (finding #4-echo)
    // and never touches the relay's broadcast cursor (finding #3).
    let _ = ws_delta_tx.send(WsDelta {
        origin,
        frame: bytes.to_vec(),
    });
}

/// Resolve a 16-byte note id to its slug, re-read the note via the store,
/// reindex derived projections, and emit `WsEvent::NoteUpdated` so the web
/// client invalidates and re-renders. Best-effort: a note whose slug can't
/// be resolved (never indexed, deleted concurrently) is skipped silently.
pub async fn emit_note_updated(
    engine: &dyn tesela_sync::SyncEngine,
    store: &FsNoteStore,
    index: &SqliteIndex,
    ws_tx: &broadcast::Sender<WsEvent>,
    note_id: [u8; 16],
) {
    let Some(slug) = slug_for_note_id(engine, note_id).await else {
        tracing::debug!(
            "ws: applied delta for unknown note id {} — skipping WsEvent",
            hex::encode(note_id)
        );
        return;
    };
    let id = NoteId::new(&slug);
    match store.get(&id).await {
        Ok(Some(note)) => {
            // Rebuild the SQL projections (search, tasks view) the same way
            // the HTTP edit path does, so web reads reflect the merged state.
            if let Err(e) = index.reindex(&note).await {
                tracing::warn!("ws: reindex after delta apply for {}: {}", slug, e);
            }
            let _ = ws_tx.send(WsEvent::NoteUpdated { note });
        }
        Ok(None) => {
            tracing::debug!(
                "ws: note {} resolved to slug {} but file missing",
                hex::encode(note_id),
                slug
            );
        }
        Err(e) => tracing::warn!("ws: re-read note {} after delta apply: {}", slug, e),
    }
}

/// Map a 16-byte note id back to its filename slug by scanning the engine's
/// Loro index (`note_id` hex → `slug`). The index is self-healing and small;
/// a linear scan per applied note is acceptable on the live-sync path.
async fn slug_for_note_id(
    engine: &dyn tesela_sync::SyncEngine,
    note_id: [u8; 16],
) -> Option<String> {
    let target = hex::encode(note_id);
    engine
        .index_entries()
        .await
        .into_iter()
        .find(|e| e.note_id == target)
        .map(|e| e.slug)
}

#[cfg(test)]
mod tests {
    //! Phase A core-logic tests: the hub's decode → apply → emit → re-broadcast
    //! path exercised at the function level against real `LoroEngine`s, with
    //! `broadcast` subscribers standing in for connected sockets. The load-
    //! bearing assertions are: a binary delta fans out, a `WsEvent::NoteUpdated`
    //! is emitted on apply (the web-as-view fix, finding #4), the server engine
    //! converges on the device's edit, and a delta is never echoed back to its
    //! origin connection (finding #4-echo). A full real-socket round-trip lives
    //! in `tests/ws_delta_round_trip.rs`.
    use super::*;
    use std::sync::Arc;
    use tesela_sync::{DeviceId, Hlc, LoroDocUpdate, LoroEngine, OpPayload, SyncEngine};

    fn note_id_for(slug: &str) -> [u8; 16] {
        let hash = blake3::hash(slug.as_bytes());
        let mut out = [0u8; 16];
        out.copy_from_slice(&hash.as_bytes()[..16]);
        out
    }

    /// A server engine that materializes to disk, plus the matching store +
    /// index reading from the same mosaic, and a separate "device" engine.
    struct Harness {
        server: LoroEngine,
        device: LoroEngine,
        store: FsNoteStore,
        index: SqliteIndex,
        _tmp: tempfile::TempDir,
    }

    async fn harness() -> Harness {
        let tmp = tempfile::tempdir().unwrap();
        let mosaic = tmp.path().to_path_buf();
        let notes_dir = mosaic.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();

        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::with_dirs(
            sdev,
            Arc::new(Hlc::new(sdev)),
            mosaic.join(".tesela").join("loro"),
            Some(notes_dir.clone()),
        )
        .await
        .unwrap();

        let ddev = DeviceId::from_bytes([0xd1; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));

        let store = FsNoteStore::new(mosaic.clone(), tesela_core::config::StorageConfig::default());
        let index = SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
            .await
            .unwrap();

        Harness {
            server,
            device,
            store,
            index,
            _tmp: tmp,
        }
    }

    /// Frame the device's full state for a note as a TLR2 binary delta.
    async fn delta_frame(device: &LoroEngine, note_id: [u8; 16]) -> Vec<u8> {
        let bytes = device.export_doc_update(note_id, None).await.unwrap();
        tesela_sync::encode_loro_relay_payload(&[LoroDocUpdate {
            doc: note_id,
            update_bytes: bytes,
        }])
        .unwrap()
    }

    #[tokio::test]
    async fn inbound_delta_converges_fans_out_and_emits_event() {
        let h = harness().await;
        let note_id = note_id_for("n");

        // Device A authors a note locally.
        h.device
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- hello from A <!-- bid:01010101-0101-0101-0101-010101010101 -->\n"
                    .into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let frame = delta_frame(&h.device, note_id).await;

        let (ws_tx, mut ws_rx) = tokio::sync::broadcast::channel::<WsEvent>(16);
        let (delta_tx, mut delta_rx) = tokio::sync::broadcast::channel::<WsDelta>(16);

        let origin: ConnId = 7;
        apply_inbound_delta(
            &h.server, &h.store, &h.index, &ws_tx, &delta_tx, &frame, Some(origin),
        )
        .await;

        // (1) Server engine converged on A's edit.
        let rendered = h.server.render_note(note_id).await.unwrap();
        assert!(rendered.contains("hello from A"), "server converged: {rendered:?}");

        // (2) A WsEvent::NoteUpdated fired (web-as-view invalidation).
        let evt = ws_rx.try_recv().expect("a WsEvent should be emitted");
        match evt {
            WsEvent::NoteUpdated { note } => {
                assert_eq!(note.id.as_str(), "n");
                assert!(note.content.contains("hello from A"));
            }
            other => panic!("expected NoteUpdated, got {other:?}"),
        }

        // (3) The exact applied bytes were re-broadcast, tagged with the
        // origin connection so the send loop can suppress the echo.
        let fanned = delta_rx.try_recv().expect("delta should fan out");
        assert_eq!(fanned.origin, Some(origin), "origin tagged for echo-suppression");
        assert_eq!(fanned.frame, frame, "exact applied bytes forwarded");
    }

    #[tokio::test]
    async fn three_node_hub_fan_out_is_finite_and_suppresses_origin() {
        // A and C send concurrent edits to the same note through the hub.
        // Assert: each apply fans out exactly one frame (finite, no infinite
        // re-broadcast), the hub converges on both edits, and the fan-out
        // carries the originating connection id so neither origin receives
        // its own frame back (echo-suppression — the send loop drops a frame
        // whose origin == its own conn id).
        let h = harness().await;
        let note_id = note_id_for("shared");

        // Seed the note on the hub + both devices so the two edits are
        // genuinely concurrent block-adds (not create races).
        let seed = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n".into(),
            created_at_millis: 1,
        };
        h.server.record_local(seed.clone()).await.unwrap();
        let base = h.server.export_doc_update(note_id, None).await.unwrap();
        h.device.import_doc_update(note_id, &base).await.unwrap();
        // Device C shares the same base.
        let cdev = DeviceId::from_bytes([0xc3; 16]);
        let device_c = LoroEngine::new(cdev, Arc::new(Hlc::new(cdev)));
        device_c.import_doc_update(note_id, &base).await.unwrap();

        // Concurrent edits: A adds a block, C adds a different block.
        h.device
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xaa; 16],
                note_id,
                parent_block_id: None,
                order_key: "a".into(),
                indent_level: 0,
                text: "A edit".into(),
            })
            .await
            .unwrap();
        device_c
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xcc; 16],
                note_id,
                parent_block_id: None,
                order_key: "c".into(),
                indent_level: 0,
                text: "C edit".into(),
            })
            .await
            .unwrap();

        let frame_a = delta_frame(&h.device, note_id).await;
        let frame_c = delta_frame(&device_c, note_id).await;

        let (ws_tx, _ws_rx) = tokio::sync::broadcast::channel::<WsEvent>(16);
        let (delta_tx, mut delta_rx) = tokio::sync::broadcast::channel::<WsDelta>(16);

        let conn_a: ConnId = 1;
        let conn_c: ConnId = 3;
        apply_inbound_delta(
            &h.server, &h.store, &h.index, &ws_tx, &delta_tx, &frame_a, Some(conn_a),
        )
        .await;
        apply_inbound_delta(
            &h.server, &h.store, &h.index, &ws_tx, &delta_tx, &frame_c, Some(conn_c),
        )
        .await;

        // Finite fan-out: exactly two frames emitted (one per apply), no
        // infinite re-broadcast.
        let f1 = delta_rx.try_recv().expect("first fan-out frame");
        let f2 = delta_rx.try_recv().expect("second fan-out frame");
        assert!(
            delta_rx.try_recv().is_err(),
            "fan-out is finite — no extra re-broadcast frames"
        );

        // Echo-suppression: the two frames carry the two distinct origins, so
        // the send loop (origin == own conn id ⇒ skip) drops A's frame for A
        // and C's frame for C. Verify the origins are exactly {conn_a, conn_c}.
        let mut origins = vec![f1.origin, f2.origin];
        origins.sort();
        assert_eq!(origins, vec![Some(conn_a), Some(conn_c)], "each fan-out tagged with its origin");

        // Hub converged on BOTH concurrent edits, no lost edit.
        let rendered = h.server.render_note(note_id).await.unwrap();
        assert!(rendered.contains("A edit"), "A's edit present: {rendered:?}");
        assert!(rendered.contains("C edit"), "C's edit present: {rendered:?}");
    }

    #[tokio::test]
    async fn non_v2_binary_frame_is_skipped_without_emit() {
        // A stray non-TLR2 binary frame must not apply, emit, or fan out.
        let h = harness().await;
        let (ws_tx, mut ws_rx) = tokio::sync::broadcast::channel::<WsEvent>(16);
        let (delta_tx, mut delta_rx) = tokio::sync::broadcast::channel::<WsDelta>(16);

        apply_inbound_delta(
            &h.server,
            &h.store,
            &h.index,
            &ws_tx,
            &delta_tx,
            b"not a tlr2 frame",
            Some(9),
        )
        .await;

        assert!(ws_rx.try_recv().is_err(), "no WsEvent for a junk frame");
        assert!(delta_rx.try_recv().is_err(), "no fan-out for a junk frame");
    }
}
