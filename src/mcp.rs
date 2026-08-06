//! Minimal MCP (Model Context Protocol) client over the Streamable HTTP transport.
//! Just enough to initialize a session, list tools, and call one: what the
//! dynamic CLI layer needs. No resumption, no server->client requests, no
//! resource/prompt support — add if a tool ever needs them.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub struct McpClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
    session_id: Option<String>,
    next_id: u64,
}

impl McpClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            http: reqwest::Client::new(),
            session_id: None,
            next_id: 1,
        }
    }

    /// Handshake: initialize, then send the required `notifications/initialized`.
    /// Must be called before list_tools/call_tool.
    pub async fn initialize(&mut self) -> Result<()> {
        let id = self.take_id();
        let (resp, session_id) = self
            .post(
                Some(id),
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": { "name": "mixio-cli", "version": env!("CARGO_PKG_VERSION") }
                }),
            )
            .await?;
        self.session_id = session_id;
        as_result(resp)?;

        // Notification: no id, no response expected.
        self.post_notification("notifications/initialized", json!({}))
            .await?;
        Ok(())
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let id = self.take_id();
        let (resp, _) = self.post(Some(id), "tools/list", json!({})).await?;
        let result = as_result(resp)?;
        let tools = result
            .get("tools")
            .cloned()
            .context("tools/list response missing `tools`")?;
        Ok(serde_json::from_value(tools)?)
    }

    /// Returns the raw `result` object (`{ content, isError, ... }`) — the caller
    /// decides how to render it.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let id = self.take_id();
        let (resp, _) = self
            .post(
                Some(id),
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await?;
        as_result(resp)
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn post_notification(&self, method: &str, params: Value) -> Result<()> {
        self.post(None, method, params).await.map(|_| ())
    }

    /// Sends one JSON-RPC request/notification and returns the parsed response
    /// (empty object for notifications) plus a session id if the server just
    /// minted one.
    async fn post(
        &self,
        id: Option<u64>,
        method: &str,
        params: Value,
    ) -> Result<(Value, Option<String>)> {
        let mut body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if let Some(id) = id {
            body["id"] = json!(id);
        }

        let mut req = self
            .http
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body);
        if let Some(sid) = &self.session_id {
            req = req.header("Mcp-Session-Id", sid);
        }

        let resp = req.send().await.context("request to MCP server failed")?;
        let status = resp.status();
        let session_id = resp
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let content_type = resp
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = resp.text().await.context("reading MCP response body")?;

        if !status.is_success() {
            bail!("MCP server returned {status}: {text}");
        }
        if id.is_none() {
            return Ok((Value::Null, session_id)); // notification: no body to parse
        }

        let parsed = if content_type.contains("text/event-stream") {
            parse_sse_json_rpc(&text)?
        } else {
            serde_json::from_str(&text).context("MCP response was not valid JSON")?
        };
        Ok((parsed, session_id))
    }
}

/// A Streamable HTTP response can come back as SSE frames (`data: {...}`)
/// instead of a bare JSON body. Take the last data event — that's the final
/// JSON-RPC response for a non-streaming tool call.
fn parse_sse_json_rpc(text: &str) -> Result<Value> {
    let mut last = None;
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            last = Some(serde_json::from_str::<Value>(data.trim())?);
        }
    }
    last.context("SSE stream had no `data:` events")
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Pulls `result` out of a JSON-RPC response, turning `error` into an Err.
fn as_result(resp: Value) -> Result<Value> {
    if let Some(err) = resp.get("error") {
        let err: JsonRpcError = serde_json::from_value(err.clone())?;
        bail!("MCP error {}: {}", err.code, err.message);
    }
    resp.get("result")
        .cloned()
        .context("JSON-RPC response missing both `result` and `error`")
}
