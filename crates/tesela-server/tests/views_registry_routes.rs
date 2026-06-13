//! HTTP-level coverage for the saved-views registry routes (saved-views
//! spec, 2026-06-10): CRUD round-trip over `/views`, the `(order, id)`
//! ordered list, the builtin delete guard, DSL validation (the editor UI
//! shows the rejection message), `/views/reorder`, the bring-up seed
//! (`ensure_builtin_views` runs after engine open, before serving — so a
//! fresh server already carries the Inbox with `INBOX_VIEW_DSL`), and the
//! `views_changed` WS event clients use to live-refresh the switcher.
//!
//! Skipped on non-Unix (spawns the server binary, SIGTERMs to shut down).

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tesela-server"))
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn wait_for_port(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    Ok(())
}

struct ServerGuard(Option<Child>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
    }
}

fn spawn_server(mosaic: &Path, addr: &str) -> ServerGuard {
    let child = Command::new(binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server");
    ServerGuard(Some(child))
}

struct Harness {
    base: String,
    _server: ServerGuard,
    _temp: TempDir,
}

fn boot() -> Harness {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    let server = spawn_server(&mosaic, &addr);
    assert!(
        wait_for_port(&addr, Duration::from_secs(60)),
        "server never bound to {addr}"
    );
    Harness {
        base,
        _server: server,
        _temp: temp,
    }
}

async fn get_views(client: &reqwest::Client, base: &str) -> Vec<serde_json::Value> {
    client
        .get(format!("{base}/views"))
        .send()
        .await
        .expect("GET /views")
        .error_for_status()
        .expect("GET /views ok")
        .json::<Vec<serde_json::Value>>()
        .await
        .expect("views json")
}

/// Bring-up seeds the builtin Inbox (idempotent `ensure_builtin_views`
/// after engine open, before serving) with tesela-core's `INBOX_VIEW_DSL`
/// verbatim; deleting it is refused with a clear 4xx error and the
/// registry is left intact.
#[tokio::test(flavor = "current_thread")]
async fn bring_up_seeds_inbox_and_builtin_delete_is_refused() {
    let h = boot();
    let client = reqwest::Client::new();

    let views = get_views(&client, &h.base).await;
    assert_eq!(views.len(), 1, "fresh server carries exactly the Inbox");
    let inbox = &views[0];
    assert_eq!(inbox["id"], "builtin-inbox");
    assert_eq!(inbox["name"], "Inbox");
    assert_eq!(
        inbox["dsl"].as_str().unwrap(),
        tesela_core::query::INBOX_VIEW_DSL,
        "seeded Inbox carries the locked core DSL verbatim"
    );
    assert_eq!(inbox["builtin"], true);
    assert_eq!(inbox["display_mode"], "list");

    // Builtin delete is refused with a clear message…
    let resp = client
        .delete(format!("{}/views/builtin-inbox", h.base))
        .send()
        .await
        .expect("DELETE builtin");
    assert_eq!(resp.status().as_u16(), 400, "builtin delete → 4xx");
    let body: serde_json::Value = resp.json().await.expect("error body");
    let msg = body["error"].as_str().expect("error message");
    assert!(
        msg.contains("builtin"),
        "error explains the builtin guard: {msg}"
    );

    // …and the Inbox is still there.
    let views = get_views(&client, &h.base).await;
    assert_eq!(views.len(), 1);
    assert_eq!(views[0]["id"], "builtin-inbox");

    // Deleting a view that doesn't exist is a 404.
    let resp = client
        .delete(format!("{}/views/no-such-view", h.base))
        .send()
        .await
        .expect("DELETE unknown");
    assert_eq!(resp.status().as_u16(), 404);
}

/// Full CRUD round-trip: create (server mints the id unless provided),
/// the list stays sorted by `(order, id)`, update mutates
/// name/dsl/display/order, reorder reassigns order values from the
/// submitted id array, and delete removes user views.
#[tokio::test(flavor = "current_thread")]
async fn views_crud_round_trip_ordered_list_and_reorder() {
    let h = boot();
    let client = reqwest::Client::new();

    // Create WITHOUT an id — the server mints one.
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "This week", "dsl": "has:scheduled" }))
        .send()
        .await
        .expect("POST /views");
    assert_eq!(resp.status().as_u16(), 201, "create → 201");
    let week: serde_json::Value = resp.json().await.expect("created json");
    let week_id = week["id"].as_str().expect("minted id").to_string();
    assert!(!week_id.is_empty() && week_id != "builtin-inbox");
    assert_eq!(
        week["builtin"], false,
        "HTTP-created views are never builtin"
    );

    // Create WITH an explicit id + display options.
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({
            "id": "v-board",
            "name": "Board",
            "dsl": "tag:project status:doing",
            "display_mode": "kanban",
            "display_group_by": "status",
        }))
        .send()
        .await
        .expect("POST /views (explicit id)");
    assert_eq!(resp.status().as_u16(), 201);
    let board: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(board["id"], "v-board");
    assert_eq!(board["display_mode"], "kanban");
    assert_eq!(board["display_group_by"], "status");

    // Creating over an existing id is refused (use PUT to update).
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "id": "v-board", "name": "Dup", "dsl": "tag:x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400, "duplicate id create → 4xx");

    // Ordered list: inbox (order 0), then the two creates appended in order.
    let views = get_views(&client, &h.base).await;
    let ids: Vec<&str> = views.iter().map(|v| v["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["builtin-inbox", week_id.as_str(), "v-board"]);

    // Update name/dsl/order/display on the explicit-id view.
    let resp = client
        .put(format!("{}/views/v-board", h.base))
        .json(&serde_json::json!({
            "name": "Project board",
            "dsl": "tag:project",
            "order": 5,
            "display_show_done": false,
        }))
        .send()
        .await
        .expect("PUT /views/v-board");
    assert_eq!(resp.status().as_u16(), 200);
    let updated: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(updated["name"], "Project board");
    assert_eq!(updated["dsl"], "tag:project");
    assert_eq!(updated["order"], 5);
    assert_eq!(updated["display_show_done"], false);
    assert_eq!(
        updated["display_group_by"], "status",
        "fields omitted from the PUT body are preserved"
    );

    // The new order resorts the list: inbox(0) < v-board(5) < week(10).
    let views = get_views(&client, &h.base).await;
    let ids: Vec<&str> = views.iter().map(|v| v["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["builtin-inbox", "v-board", week_id.as_str()]);

    // Updating an unknown view is a 404.
    let resp = client
        .put(format!("{}/views/no-such-view", h.base))
        .json(&serde_json::json!({ "name": "X" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404);

    // Reorder: the submitted array becomes the new order.
    let resp = client
        .post(format!("{}/views/reorder", h.base))
        .json(&serde_json::json!([week_id, "builtin-inbox", "v-board"]))
        .send()
        .await
        .expect("POST /views/reorder");
    assert_eq!(resp.status().as_u16(), 200);
    let reordered: Vec<serde_json::Value> = resp.json().await.unwrap();
    let ids: Vec<&str> = reordered
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![week_id.as_str(), "builtin-inbox", "v-board"]);
    // …and GET agrees (the order values persisted).
    let views = get_views(&client, &h.base).await;
    let ids: Vec<&str> = views.iter().map(|v| v["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec![week_id.as_str(), "builtin-inbox", "v-board"]);

    // Reorder with an unknown id is refused wholesale.
    let resp = client
        .post(format!("{}/views/reorder", h.base))
        .json(&serde_json::json!(["builtin-inbox", "ghost"]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);

    // Delete a user view; a second delete of the same id is a 404.
    let resp = client
        .delete(format!("{}/views/{}", h.base, week_id))
        .send()
        .await
        .expect("DELETE user view");
    assert_eq!(resp.status().as_u16(), 200);
    let resp = client
        .delete(format!("{}/views/{}", h.base, week_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404, "double delete → 404");
    let views = get_views(&client, &h.base).await;
    assert_eq!(views.len(), 2, "inbox + v-board remain");
}

/// DSL validation on create/update: unparseable DSL (the liberal parser
/// recognized no predicates) is rejected with the message the editor UI
/// surfaces; valid DSL — including the comma-OR Inbox shape — passes.
#[tokio::test(flavor = "current_thread")]
async fn invalid_dsl_is_rejected_with_the_parse_message() {
    let h = boot();
    let client = reqwest::Client::new();

    // Barewords with no key:value shape parse to zero predicates → 400,
    // and the body carries the offending DSL so the editor can show it.
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "Bad", "dsl": "hello world" }))
        .send()
        .await
        .expect("POST bad dsl");
    assert_eq!(resp.status().as_u16(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    let msg = body["error"].as_str().expect("error message");
    assert!(
        msg.contains("hello world"),
        "message names the rejected DSL: {msg}"
    );

    // Empty DSL → 400.
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "Empty", "dsl": "   " }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);

    // Unknown display_mode → 400.
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "Grid", "dsl": "tag:x", "display_mode": "grid" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);

    // PUT with garbage DSL is rejected and the stored view is untouched.
    let resp = client
        .put(format!("{}/views/builtin-inbox", h.base))
        .json(&serde_json::json!({ "dsl": "@@@@" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);
    let views = get_views(&client, &h.base).await;
    assert_eq!(
        views[0]["dsl"].as_str().unwrap(),
        tesela_core::query::INBOX_VIEW_DSL,
        "rejected PUT must not mutate the view"
    );

    // Positive control: a valid DSL is accepted on create…
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "OK", "dsl": "status:todo -has:deadline" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 201);
    // …and the comma-OR shape is accepted on update (builtins are editable).
    let resp = client
        .put(format!("{}/views/builtin-inbox", h.base))
        .json(&serde_json::json!({ "dsl": "status:backlog,todo -has:scheduled" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

/// Any views change emits a `views_changed` WS event (mirroring
/// `note_updated`'s text-JSON fan-out) so connected clients live-refresh
/// the view switcher.
#[tokio::test(flavor = "current_thread")]
async fn views_change_emits_ws_event() {
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::Message as TMessage;

    let h = boot();
    let ws_url = format!("ws://{}/ws", h.base.trim_start_matches("http://"));
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("ws connect");
    // Let the subscription register on the broadcast bus.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/views", h.base))
        .json(&serde_json::json!({ "name": "Live", "dsl": "tag:live" }))
        .send()
        .await
        .expect("POST /views");
    assert_eq!(resp.status().as_u16(), 201);

    let mut got_event = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !got_event && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
            Ok(Some(Ok(TMessage::Text(t)))) => {
                if t.contains("views_changed") {
                    assert!(
                        t.contains("\"Live\"") && t.contains("builtin-inbox"),
                        "event carries the full ordered registry: {t}"
                    );
                    got_event = true;
                }
            }
            // Binary frames (the views-doc delta fan-out) and other text
            // events may interleave; keep reading.
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }
    assert!(got_event, "a views_changed WS event must fire on create");
}
