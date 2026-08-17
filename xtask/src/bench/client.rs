use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use lsp_types::Url;
use serde_json::{Value, json};

use super::servers::ServerSpec;

#[derive(Debug)]
struct ContentModified;

impl std::fmt::Display for ContentModified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("content modified")
    }
}

impl std::error::Error for ContentModified {}

pub struct LspClient {
    pub child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    next_id: i64,
}

impl LspClient {
    pub fn spawn(server: &ServerSpec, workspace: &Path) -> Result<Self> {
        let mut child = Command::new(&server.bin)
            .current_dir(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {}", server.bin.display()))?;
        let stdout = child.stdout.take().context("server stdout missing")?;
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if !line.is_empty() {
                        eprintln!("    [server] {line}");
                    }
                }
            });
        }
        let stdin = child.stdin.take().context("server stdin missing")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Err(error) = read_loop(stdout, tx) {
                eprintln!("    [lsp read] {error:#}");
            }
        });
        Ok(Self { child, stdin, rx, next_id: 1 })
    }

    pub fn initialize(&mut self, workspace: &Path) -> Result<Value> {
        let uri = path_url(workspace)?;
        let params = json!({
            "processId": std::process::id(),
            "rootUri": uri,
            "capabilities": {
                "workspace": { "workspaceFolders": true },
                "textDocument": {
                    "definition": { "linkSupport": true },
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "references": {},
                    "completion": { "completionItem": { "snippetSupport": true } }
                }
            },
            "workspaceFolders": [{ "uri": uri, "name": workspace.file_name().and_then(|n| n.to_str()).unwrap_or("ws") }],
            "initializationOptions": {
                "files": { "watcher": "client" }
            }
        });
        let result = self.request("initialize", params)?;
        self.notify("initialized", json!({}))?;
        Ok(result)
    }

    pub fn did_open(&mut self, path: &Path, text: &str) -> Result<()> {
        let uri = path_url(path)?;
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id(path),
                    "version": 1,
                    "text": text
                }
            }),
        )
    }

    pub fn did_change(&mut self, path: &Path, version: i32, text: &str) -> Result<()> {
        let uri = path_url(path)?;
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        )
    }

    pub fn request_at(
        &mut self,
        method: &str,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<Value> {
        let uri = path_url(path)?;
        let position = json!({ "line": line, "character": character });
        let text_document = json!({ "uri": uri });
        let params = match method {
            "textDocument/definition" | "textDocument/hover" | "textDocument/completion" => {
                json!({ "textDocument": text_document, "position": position })
            }
            "textDocument/references" => json!({
                "textDocument": text_document,
                "position": position,
                "context": { "includeDeclaration": true }
            }),
            other => bail!("unsupported method {other}"),
        };
        self.request(method, params)
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let _ = self.request("shutdown", json!(null));
        let _ = self.notify("exit", json!(null));
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        const ATTEMPTS: usize = 12;
        let mut last_modified = None;
        for attempt in 0..ATTEMPTS {
            let id = self.next_id;
            self.next_id += 1;
            let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
            write_message(&mut self.stdin, &message)?;
            match self.wait_response(id, Duration::from_secs(180)) {
                Ok(result) => return Ok(result),
                Err(error) if error.is::<ContentModified>() => {
                    last_modified = Some(error);
                    thread::sleep(Duration::from_millis(50 * (attempt as u64 + 1)));
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_modified.unwrap_or_else(|| anyhow::anyhow!("content modified")))
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        write_message(&mut self.stdin, &message)
    }

    fn wait_response(&mut self, id: i64, timeout: Duration) -> Result<Value> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                bail!("timed out waiting for response {id}");
            }
            let message = self.rx.recv_timeout(remaining).context("server closed while waiting")?;
            if message.get("method").is_some() && message.get("id").is_some() {
                let reply_id = message.get("id").cloned().unwrap_or(Value::Null);
                let _ = write_message(
                    &mut self.stdin,
                    &json!({ "jsonrpc": "2.0", "id": reply_id, "result": null }),
                );
                continue;
            }
            if message.get("id").and_then(Value::as_i64) == Some(id) {
                if let Some(error) = message.get("error") {
                    if error.get("code").and_then(Value::as_i64) == Some(-32801) {
                        return Err(ContentModified.into());
                    }
                    bail!("LSP error: {error}");
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_message(stdin: &mut ChildStdin, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len())?;
    stdin.write_all(&body)?;
    stdin.flush()?;
    Ok(())
}

fn read_loop(reader: impl Read, tx: mpsc::Sender<Value>) -> Result<()> {
    let mut reader = BufReader::new(reader);
    loop {
        let Some(message) = read_message(&mut reader)? else {
            return Ok(());
        };
        if tx.send(message).is_err() {
            return Ok(());
        }
    }
}

fn read_message(reader: &mut BufReader<impl Read>) -> Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse::<usize>().context("invalid Content-Length")?);
        }
    }
    let Some(len) = content_length else {
        bail!("LSP message missing Content-Length");
    };
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

pub fn path_url(path: &Path) -> Result<Url> {
    Url::from_file_path(path).map_err(|()| anyhow::anyhow!("invalid file path {}", path.display()))
}

fn language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("svh" | "sv") => "systemverilog",
        _ => "verilog",
    }
}
