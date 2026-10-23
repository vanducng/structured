use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use assert_cmd::Command;
use serde_json::{Value, json};

type Responder = dyn Fn(&str, &Value) -> Value + Send + Sync;

/// A tiny MCP endpoint: answers tools/call with `respond` and records every call.
pub struct Mock {
    url: String,
    config: PathBuf,
    calls: Arc<Mutex<Vec<Value>>>,
}

impl Mock {
    pub fn start(respond: impl Fn(&str, &Value) -> Value + Send + Sync + 'static) -> Self {
        static RUN: AtomicUsize = AtomicUsize::new(0);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/mcp", listener.local_addr().expect("addr"));
        let config = std::env::temp_dir().join(format!(
            "structured-mock-{}-{}",
            std::process::id(),
            RUN.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(config.join("structured")).expect("config dir");
        std::fs::write(
            config.join("structured/auth.json"),
            r#"{"client_id":"t","access_token":"t","refresh_token":"","expires_at":4102444800}"#,
        )
        .expect("auth");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let respond: Arc<Responder> = Arc::new(respond);
        let log = Arc::clone(&calls);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (respond, log) = (Arc::clone(&respond), Arc::clone(&log));
                thread::spawn(move || serve(stream, &*respond, &log));
            }
        });
        Self { url, config, calls }
    }

    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("structured").expect("binary");
        cmd.env("XDG_CONFIG_HOME", &self.config)
            .env("STRUCTURED_MCP_URL", &self.url);
        cmd
    }

    pub fn calls(&self) -> Vec<Value> {
        self.calls.lock().expect("calls").clone()
    }

    pub fn call(&self, tool: &str) -> Value {
        self.calls()
            .into_iter()
            .find(|call| call["tool"] == tool)
            .unwrap_or_else(|| panic!("no {tool} call in {:?}", self.calls()))["arguments"]
            .clone()
    }
}

fn serve(stream: TcpStream, respond: &Responder, log: &Mutex<Vec<Value>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    loop {
        let mut length = 0;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).expect("body");
        let request: Value = serde_json::from_slice(&body).expect("json body");
        let Some(id) = request.get("id").cloned() else {
            let _ = writer.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n");
            continue;
        };
        let result = if request["method"] == "initialize" {
            json!({"protocolVersion": "2025-03-26", "capabilities": {}})
        } else {
            let tool = request["params"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let arguments = request["params"]["arguments"].clone();
            log.lock()
                .expect("log")
                .push(json!({"tool": tool, "arguments": arguments}));
            let data = respond(&tool, &arguments);
            match data.get("isError") {
                Some(_) => {
                    json!({"isError": true, "content": [{"type": "text", "text": data["text"]}]})
                }
                None => {
                    json!({"content": [{"type": "text", "text": data.to_string()}], "structuredContent": data})
                }
            }
        };
        let payload = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        );
        let _ = writer.write_all(head.as_bytes());
        let _ = writer.write_all(payload.as_bytes());
    }
}
