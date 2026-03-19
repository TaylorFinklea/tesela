//! Thin JSON-RPC 2.0 transport over stdin/stdout

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }

    pub fn method_not_found(id: Option<Value>, method: &str) -> Self {
        Self::error(id, -32601, format!("Method not found: {}", method))
    }

    pub fn internal_error(id: Option<Value>, message: String) -> Self {
        Self::error(id, -32603, message)
    }

    pub fn parse_error() -> Self {
        Self::error(None, -32700, "Parse error".to_string())
    }

    pub fn invalid_params(id: Option<Value>, message: String) -> Self {
        Self::error(id, -32602, message)
    }
}

/// Read one JSON-RPC request from stdin line-by-line.
pub async fn read_request(reader: &mut BufReader<tokio::io::Stdin>) -> Option<JsonRpcRequest> {
    let mut line = String::new();
    match reader.read_line(&mut line).await {
        Ok(0) => None, // EOF
        Ok(_) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            match serde_json::from_str(trimmed) {
                Ok(req) => Some(req),
                Err(e) => {
                    tracing::warn!("Failed to parse JSON-RPC request: {}", e);
                    None // caller handles parse error
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to read from stdin: {}", e);
            None
        }
    }
}

/// Write a JSON-RPC response to stdout.
pub async fn write_response(
    writer: &mut tokio::io::Stdout,
    response: &JsonRpcResponse,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(response)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}
