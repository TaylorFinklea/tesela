use anyhow::Result;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use tesela_core::{
    config::Config,
    db::SqliteIndex,
    storage::filesystem::FsNoteStore,
};
use tesela_mcp::{
    tools::{list_tools, ToolRegistry},
    transport::{read_request, write_response, JsonRpcRequest, JsonRpcResponse},
};
use tokio::io::{stdin, stdout, BufReader};

#[tokio::main]
async fn main() -> Result<()> {
    // Log to stderr (stdout is reserved for MCP protocol)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mosaic = find_mosaic()?;
    let db_path = mosaic.join(".tesela").join("tesela.db");

    let config_path = mosaic.join(".tesela").join("config.toml");
    let config = Config::load_or_default(&config_path);

    let store = Arc::new(FsNoteStore::new(mosaic, config.storage));
    let index = Arc::new(SqliteIndex::open(&db_path).await?);
    let registry = Arc::new(ToolRegistry::new(store, index));

    tracing::info!("tesela-mcp server started");

    let mut reader = BufReader::new(stdin());
    let mut writer = stdout();

    loop {
        let request = match read_request(&mut reader).await {
            Some(req) => req,
            None => break, // EOF
        };

        tracing::debug!("Received: {} (id: {:?})", request.method, request.id);

        let response = handle_request(&registry, request).await;
        write_response(&mut writer, &response).await?;
    }

    tracing::info!("tesela-mcp server shutting down");
    Ok(())
}

async fn handle_request(registry: &ToolRegistry, req: JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            req.id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "tesela",
                    "version": "0.1.0"
                }
            }),
        ),
        "tools/list" => JsonRpcResponse::success(req.id, list_tools()),
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let name = match params["name"].as_str() {
                Some(n) => n.to_string(),
                None => {
                    return JsonRpcResponse::invalid_params(
                        req.id,
                        "Missing 'name' field".to_string(),
                    )
                }
            };
            let tool_params = params.get("arguments").cloned();

            match registry.call(&name, tool_params).await {
                Ok(result) => JsonRpcResponse::success(req.id, result),
                Err(e) => JsonRpcResponse::internal_error(req.id, e),
            }
        }
        "notifications/initialized" => {
            // No response needed for notifications, but send empty success if id present
            JsonRpcResponse::success(req.id, json!({}))
        }
        "ping" => JsonRpcResponse::success(req.id, json!({})),
        _ => JsonRpcResponse::method_not_found(req.id, &req.method),
    }
}

fn find_mosaic() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".tesela").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("No mosaic found. Run 'tesela init' first.")
}
