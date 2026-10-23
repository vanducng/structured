use reqwest::blocking::Client;
use serde_json::{Value, json};

use crate::auth::{self, Auth};
use crate::error::{Error, Result};
use crate::model::{self, Task};

const MCP_URL: &str = "https://mcp.structured.app/mcp";

fn endpoint() -> String {
    std::env::var("STRUCTURED_MCP_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| MCP_URL.to_owned())
}

pub struct Session {
    http: Client,
    auth: Auth,
    protocol: String,
    session_id: Option<String>,
    next_id: u64,
    ready: bool,
    pub dry_run: bool,
}

const WRITES: [&str; 5] = [
    "create_task",
    "create_recurring_task",
    "update_task",
    "complete_task",
    "delete_task",
];

impl Session {
    pub fn connect() -> Result<Self> {
        let http = auth::http()?;
        let auth = auth::load()?;
        if auth.refresh_token.is_empty() && auth.access_token.is_empty() {
            return Err(Error::Auth);
        }
        Ok(Self {
            http,
            auth,
            protocol: "2025-03-26".into(),
            session_id: None,
            next_id: 1,
            ready: false,
            dry_run: false,
        })
    }

    pub fn login() -> Result<()> {
        let http = auth::http()?;
        auth::login(&http)?;
        Ok(())
    }

    fn ensure(&mut self) -> Result<()> {
        if self.ready {
            return Ok(());
        }
        let result = self.rpc(
            "initialize",
            Some(json!({
                "protocolVersion": self.protocol,
                "capabilities": {},
                "clientInfo": {"name": "structured", "version": env!("CARGO_PKG_VERSION")},
            })),
        )?;
        if let Some(version) = result.get("protocolVersion").and_then(Value::as_str) {
            self.protocol = version.to_owned();
        }
        self.notify("notifications/initialized")?;
        self.ready = true;
        Ok(())
    }

    pub fn call(&mut self, name: &str, arguments: Value) -> Result<Value> {
        if self.dry_run && WRITES.contains(&name) {
            return Ok(
                json!({"structuredContent": {"dry_run": true, "tool": name, "arguments": arguments}}),
            );
        }
        self.ensure()?;
        self.rpc(
            "tools/call",
            Some(json!({"name": name, "arguments": arguments})),
        )
    }

    pub fn tasks(&mut self, name: &str, arguments: Value) -> Result<Vec<Task>> {
        let result = self.call(name, arguments)?;
        model::tasks_from_tool(&result)
    }

    fn notify(&mut self, method: &str) -> Result<()> {
        let _ = self.rpc_raw(method, None, true)?;
        Ok(())
    }

    fn rpc(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let message = self.rpc_raw(method, params, false)?;
        if let Some(error) = message.get("error") {
            let text = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("MCP error");
            let lower = text.to_ascii_lowercase();
            if lower.contains("unauthorized") || lower.contains("not authenticated") {
                return Err(Error::Auth);
            }
            return Err(Error::Api(text.to_owned()));
        }
        Ok(message.get("result").cloned().unwrap_or(Value::Null))
    }

    fn rpc_raw(&mut self, method: &str, params: Option<Value>, note: bool) -> Result<Value> {
        let token = auth::bearer(&self.http, &mut self.auth)?;
        let mut body = json!({"jsonrpc": "2.0", "method": method});
        if !note {
            body["id"] = json!(self.next_id);
            self.next_id += 1;
        }
        if let Some(params) = params {
            body["params"] = params;
        }
        let mut request = self
            .http
            .post(endpoint())
            .bearer_auth(token)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", &self.protocol)
            .json(&body);
        if let Some(session) = &self.session_id {
            request = request.header("Mcp-Session-Id", session);
        }
        let response = request.send()?;
        if let Some(session) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.session_id = Some(session.to_owned());
        }
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err(Error::Auth);
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(Error::Api(format!("HTTP {status}: {text}")));
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if response.status().as_u16() == 202 {
            return Ok(Value::Null);
        }
        let bytes = response.bytes()?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        parse_message(&content_type, &bytes)
    }
}

fn parse_message(content_type: &str, bytes: &[u8]) -> Result<Value> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    if text.is_empty() {
        return Ok(Value::Null);
    }
    if content_type.contains("text/event-stream")
        || text.starts_with("data:")
        || text.starts_with("event:")
    {
        let mut last = Value::Null;
        for block in text.split("\n\n") {
            let data: String = block
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                last = serde_json::from_str(&data)?;
            }
        }
        return Ok(last);
    }
    Ok(serde_json::from_str(text)?)
}
