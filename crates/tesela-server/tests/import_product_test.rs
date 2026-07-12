#![cfg(unix)]

use std::fs;
use std::net::SocketAddr;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::time::Duration;

use tempfile::TempDir;
use tesela_core::import_logseq::{ApplyDecisions, ApplyOutcome, ImportPlan, PlanKind};
use tesela_core::stable_uuid_from_slug;
use tesela_server::{serve, ServeConfig};

fn make_mosaic(root: &Path) {
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::create_dir_all(root.join("attachments")).unwrap();
    fs::create_dir_all(root.join(".tesela")).unwrap();
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )
    .unwrap();
}

fn write_graph(root: &Path, count: usize, prefix: &str) {
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    fs::write(
        pages.join(format!("{prefix} Feature.md")),
        "title:: Feature\n# Imported heading\n\nImported prose\n\n```query\n{:find [?b]}\n- literal bullet\n```\n",
    )
    .unwrap();
    for i in 1..count {
        fs::write(
            pages.join(format!("{prefix} Page {i:03}.md")),
            format!("- imported page {i}\n"),
        )
        .unwrap();
    }
}

async fn boot_server(
    mosaic: &Path,
) -> (
    SocketAddr,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
) {
    std::env::set_var("TESELA_SERVER_BIND", "127.0.0.1:0");
    std::env::set_var("TESELA_DISABLE_MDNS", "1");
    std::env::set_var("TESELA_DISABLE_PEER_SYNC", "1");
    std::env::set_var("TESELA_GROUP_KEY_FILE_STORE", "1");
    let config = ServeConfig::resolve(Some(mosaic.to_path_buf())).unwrap();
    let (bound_tx, bound_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        serve(
            config,
            async move {
                let _ = shutdown_rx.await;
            },
            move |addr| {
                let _ = bound_tx.send(addr);
            },
        )
        .await
    });
    let addr = tokio::time::timeout(Duration::from_secs(20), bound_rx)
        .await
        .expect("server binds")
        .expect("bound address");
    (addr, shutdown_tx, handle)
}

fn assert_lock_available(mosaic: &Path) {
    let path = mosaic.join(".tesela/server.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .unwrap();
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(rc, 0, "temporary import releases its mosaic lock");
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
}

#[tokio::test(flavor = "multi_thread")]
async fn active_temporary_and_create_imports_write_through_the_correct_engine() {
    let temp = TempDir::new().unwrap();
    let active = temp.path().join("active");
    let active_graph = temp.path().join("active-graph");
    make_mosaic(&active);
    write_graph(&active_graph, 501, "Active");
    let (addr, shutdown, server) = boot_server(&active).await;
    let client = reqwest::Client::new();

    let plan_response = client
        .post(format!("http://{addr}/imports/logseq/plan"))
        .json(&serde_json::json!({ "source": active_graph }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let plan: ImportPlan = plan_response.json().await.unwrap();
    assert_eq!(plan.items.len(), 501);
    let apply_response = tokio::time::timeout(
        Duration::from_secs(20),
        client
            .post(format!("http://{addr}/imports/logseq/apply"))
            .json(&serde_json::json!({
                "plan": plan,
                "decisions": ApplyDecisions::default()
            }))
            .send(),
    )
    .await
    .expect("HTTP import stays inside request budget")
    .unwrap()
    .error_for_status()
    .unwrap();
    let applied: ApplyOutcome = apply_response.json().await.unwrap();
    assert_eq!(applied.imported, 501);
    assert!(applied.errors.is_empty(), "{:?}", applied.errors);

    let index: serde_json::Value = client
        .get(format!("http://{addr}/loro/index"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(index.as_array().unwrap().len(), 501);
    let active_slug = "active-feature";
    let active_file = active.join(format!("notes/{active_slug}.md"));
    let active_markdown = fs::read_to_string(&active_file).unwrap();
    assert!(active_markdown.contains("# Imported heading"));
    assert!(active_markdown.contains("```query"));
    assert!(active
        .join(format!(
            ".tesela/loro/{}.bin",
            hex::encode(stable_uuid_from_slug(active_slug))
        ))
        .exists());

    let second_plan: ImportPlan = client
        .post(format!("http://{addr}/imports/logseq/plan"))
        .json(&serde_json::json!({ "source": active_graph }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(second_plan
        .items
        .iter()
        .all(|item| item.kind == PlanKind::Unchanged));
    let second: ApplyOutcome = client
        .post(format!("http://{addr}/imports/logseq/apply"))
        .json(&serde_json::json!({
            "plan": second_plan,
            "decisions": ApplyDecisions::default()
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second.unchanged, 501);

    let legacy: serde_json::Value = client
        .post(format!("http://{addr}/imports/logseq"))
        .json(&serde_json::json!({ "source": active_graph, "dry_run": false }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(legacy["success"], true);
    assert!(legacy["stdout"].as_str().unwrap().contains("Unchanged"));

    let other = temp.path().join("other");
    let other_graph = temp.path().join("other-graph");
    make_mosaic(&other);
    write_graph(&other_graph, 1, "Other");
    let other_plan: ImportPlan = client
        .post(format!("http://{addr}/imports/logseq/plan"))
        .json(&serde_json::json!({ "source": other_graph, "mosaic": other }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let other_outcome: ApplyOutcome = client
        .post(format!("http://{addr}/imports/logseq/apply"))
        .json(&serde_json::json!({
            "plan": other_plan,
            "decisions": ApplyDecisions::default(),
            "mosaic": other
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(other_outcome.imported, 1);
    assert!(other.join("notes/other-feature.md").exists());
    assert!(!active.join("notes/other-feature.md").exists());
    assert_lock_available(&other);

    let created = temp.path().join("created");
    let created_graph = temp.path().join("created-graph");
    write_graph(&created_graph, 1, "Created");
    let created_response: serde_json::Value = client
        .post(format!("http://{addr}/mosaics"))
        .json(&serde_json::json!({
            "path": created,
            "import": { "kind": "logseq", "source": created_graph }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created_response["import_success"], true);
    assert!(created.join("notes/created-feature.md").exists());
    assert_lock_available(&created);

    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(20), server)
        .await
        .expect("server stops")
        .expect("server task")
        .expect("clean shutdown");
}
