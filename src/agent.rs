use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde_json::{Map, Value, json};
use toml_edit::{Array, DocumentMut, Item, Table, Value as TomlValue, value};
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum AgentClient {
    Claude,
    Codex,
    Cursor,
}

impl AgentClient {
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
        }
    }

    fn executable_name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }

    fn config_path(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude.json"),
            Self::Codex => home.join(".codex").join("config.toml"),
            Self::Cursor => home.join(".cursor").join("mcp.json"),
        }
    }

    fn install_hint(self) -> &'static str {
        match self {
            Self::Claude => "Install Claude Code, then rerun `ig agent install claude`.",
            Self::Codex => "Install Codex, then rerun `ig agent install codex`.",
            Self::Cursor => "Install Cursor, then rerun `ig agent install cursor`.",
        }
    }
}

#[derive(Debug)]
struct ClientStatus {
    client: AgentClient,
    detected_by: Option<String>,
    config: Result<bool>,
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct SmokeWorkspace {
    root: PathBuf,
}

impl SmokeWorkspace {
    fn create() -> Result<(Self, String)> {
        let root = env::temp_dir().join(format!("ivygrep-agent-smoke-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join(".git"))?;
        fs::create_dir_all(root.join("src"))?;
        let probe = format!("ivygrep_agent_probe_{}", Uuid::new_v4().simple());
        fs::write(
            root.join("src").join("probe.rs"),
            format!("pub fn {probe}() -> bool {{ true }}\n"),
        )?;
        Ok((Self { root }, probe))
    }
}

impl Drop for SmokeWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn install(client: AgentClient) -> Result<()> {
    let home = agent_home()?;
    let Some(detected_by) = detect_client(client, &home) else {
        bail!("{} not detected. {}", client.label(), client.install_hint());
    };
    let executable = current_executable()?;
    let smoke = verify_mcp(&executable)?;
    let (path, changed) = write_client_config(client, &home, &executable)?;
    if !config_matches(client, &home, &executable)? {
        bail!(
            "{} config was written but did not validate. Inspect {} and rerun `ig agent install {}`.",
            client.label(),
            path.display(),
            client.executable_name()
        );
    }

    println!("✓ {} detected: {detected_by}", client.label());
    println!(
        "✓ {} {}: {}",
        if changed {
            "Config updated"
        } else {
            "Config already current"
        },
        client.label(),
        path.display()
    );
    println!("✓ MCP handshake: {}", smoke.protocol_version);
    println!(
        "✓ Real search: {} result(s) in {} ms",
        smoke.result_count, smoke.elapsed_ms
    );
    println!("Ready. Restart {} if it is already open.", client.label());
    Ok(())
}

pub fn doctor() -> Result<()> {
    let home = agent_home()?;
    let executable = current_executable()?;
    let clients = [AgentClient::Claude, AgentClient::Codex, AgentClient::Cursor]
        .into_iter()
        .map(|client| ClientStatus {
            client,
            detected_by: detect_client(client, &home),
            config: config_matches(client, &home, &executable),
        })
        .collect::<Vec<_>>();

    let mut configured = 0usize;
    for status in &clients {
        match (&status.detected_by, &status.config) {
            (Some(detected_by), Ok(true)) => {
                configured += 1;
                println!("✓ {}: configured ({detected_by})", status.client.label());
            }
            (Some(detected_by), Ok(false)) => println!(
                "! {}: detected ({detected_by}), not configured. Run `ig agent install {}`.",
                status.client.label(),
                status.client.executable_name()
            ),
            (Some(detected_by), Err(error)) => println!(
                "✗ {}: detected ({detected_by}), config invalid: {error:#}. Run `ig agent install {}` after fixing malformed config.",
                status.client.label(),
                status.client.executable_name()
            ),
            (None, _) => println!("- {}: not detected", status.client.label()),
        }
    }

    if configured == 0 {
        bail!(
            "no detected agent has a working ivygrep MCP entry. Run `ig agent install claude`, `ig agent install codex`, or `ig agent install cursor`."
        );
    }

    let smoke = verify_mcp(&executable)?;
    println!("✓ MCP handshake: {}", smoke.protocol_version);
    println!("✓ Tool discovery: ig_search");
    println!(
        "✓ Real search: {} result(s) in {} ms",
        smoke.result_count, smoke.elapsed_ms
    );
    println!("Agent setup healthy.");
    Ok(())
}

fn agent_home() -> Result<PathBuf> {
    env::var_os("IVYGREP_AGENT_HOME")
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .context("cannot locate home directory")
}

#[derive(Debug)]
struct SmokeResult {
    protocol_version: String,
    result_count: u64,
    elapsed_ms: u128,
}

fn verify_mcp(executable: &Path) -> Result<SmokeResult> {
    let started = Instant::now();
    let (workspace, probe) = SmokeWorkspace::create()?;
    let mut child = ChildGuard(
        Command::new(executable)
            .arg("--mcp")
            .current_dir(&workspace.root)
            .env("IVYGREP_HOME", workspace.root.join("ivygrep-home"))
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot start `{}` --mcp", executable.display()))?,
    );
    let mut stdin = child.0.stdin.take().context("MCP stdin unavailable")?;
    let stdout = child.0.stdout.take().context("MCP stdout unavailable")?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender
                .send(line.map_err(|error| error.to_string()))
                .is_err()
            {
                break;
            }
        }
    });

    send_message(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "ivygrep-agent-doctor", "version": env!("CARGO_PKG_VERSION")}
            }
        }),
    )?;
    let initialize = receive_response(&receiver, 1)?;
    let protocol_version = initialize["result"]["protocolVersion"]
        .as_str()
        .context("MCP initialize response omitted protocolVersion")?
        .to_string();
    send_message(
        &mut stdin,
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    )?;
    send_message(
        &mut stdin,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )?;
    let tools = receive_response(&receiver, 2)?;
    let has_search = tools["result"]["tools"]
        .as_array()
        .is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "ig_search"));
    if !has_search {
        bail!("MCP handshake succeeded, but `ig_search` was not advertised");
    }

    send_message(
        &mut stdin,
        json!({
            "jsonrpc":"2.0",
            "id":3,
            "method":"tools/call",
            "params": {
                "name":"ig_search",
                "arguments": {
                    "query": probe,
                    "path": workspace.root,
                    "literal": true,
                    "limit": 1
                }
            }
        }),
    )?;
    let search = receive_response(&receiver, 3)?;
    let payload_text = search["result"]["content"][0]["text"]
        .as_str()
        .context("ig_search response omitted text payload")?;
    let payload: Value = serde_json::from_str(payload_text).context("invalid ig_search payload")?;
    let result_count = payload["result_count"]
        .as_u64()
        .context("ig_search response omitted result_count")?;
    if result_count == 0
        || !payload["results"].as_array().is_some_and(|results| {
            results
                .iter()
                .any(|result| result["file_path"] == "src/probe.rs")
        })
    {
        bail!("MCP handshake succeeded, but real search did not return src/probe.rs");
    }

    drop(stdin);
    wait_for_exit(&mut child.0, Duration::from_secs(5))?;
    Ok(SmokeResult {
        protocol_version,
        result_count,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn send_message(stdin: &mut ChildStdin, message: Value) -> Result<()> {
    writeln!(stdin, "{message}").context("failed to write MCP request")?;
    stdin.flush().context("failed to flush MCP request")
}

fn receive_response(
    receiver: &Receiver<std::result::Result<String, String>>,
    id: u64,
) -> Result<Value> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = receiver
            .recv_timeout(remaining)
            .with_context(|| {
                format!(
                    "MCP response {id} timed out after {}s",
                    RESPONSE_TIMEOUT.as_secs()
                )
            })?
            .map_err(anyhow::Error::msg)?;
        let response: Value = serde_json::from_str(&line).context("MCP returned invalid JSON")?;
        if response["id"].as_u64() != Some(id) {
            continue;
        }
        if let Some(error) = response.get("error") {
            bail!("MCP request {id} failed: {error}");
        }
        return Ok(response);
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("MCP server exited with {status}");
        }
        if Instant::now() >= deadline {
            bail!("MCP server did not exit after stdin closed");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn current_executable() -> Result<PathBuf> {
    let current = env::current_exe()
        .context("cannot resolve current ig executable")?
        .canonicalize()
        .context("cannot canonicalize current ig executable")?;
    if let Some(path_entry) = find_executable("ig")
        && path_entry
            .canonicalize()
            .is_ok_and(|resolved| resolved == current)
    {
        return Ok(path_entry);
    }
    Ok(current)
}

fn detect_client(client: AgentClient, home: &Path) -> Option<String> {
    if let Some(path) = find_executable(client.executable_name()) {
        return Some(path.display().to_string());
    }
    for path in known_client_paths(client, home) {
        if path.exists() {
            return Some(path.display().to_string());
        }
    }
    client
        .config_path(home)
        .exists()
        .then(|| format!("{} exists", client.config_path(home).display()))
}

fn known_client_paths(client: AgentClient, home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    match client {
        AgentClient::Claude => {}
        AgentClient::Codex => {
            #[cfg(target_os = "macos")]
            paths.push(PathBuf::from("/Applications/Codex.app"));
            #[cfg(target_os = "windows")]
            if let Some(local) = env::var_os("LOCALAPPDATA") {
                paths.push(PathBuf::from(local).join("Programs").join("Codex"));
            }
        }
        AgentClient::Cursor => {
            #[cfg(target_os = "macos")]
            paths.push(PathBuf::from("/Applications/Cursor.app"));
            #[cfg(target_os = "windows")]
            if let Some(local) = env::var_os("LOCALAPPDATA") {
                paths.push(
                    PathBuf::from(local)
                        .join("Programs")
                        .join("cursor")
                        .join("Cursor.exe"),
                );
            }
            #[cfg(target_os = "linux")]
            paths.push(PathBuf::from("/usr/bin/cursor"));
        }
    }
    if client.config_path(home).exists() {
        paths.push(client.config_path(home));
    }
    paths
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    #[cfg(windows)]
    let extensions = env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".EXE".to_string(), ".CMD".to_string(), ".BAT".to_string()]);
    for directory in env::split_paths(&path) {
        #[cfg(not(windows))]
        {
            let candidate = directory.join(name);
            if is_executable_file(&candidate) {
                return Some(absolute_path(candidate));
            }
        }
        #[cfg(windows)]
        for extension in &extensions {
            let candidate = directory.join(format!("{name}{extension}"));
            if is_executable_file(&candidate) {
                return Some(absolute_path(candidate));
            }
        }
    }
    None
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        env::current_dir().map_or(path.clone(), |current| current.join(path))
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    return metadata.permissions().mode() & 0o111 != 0;
    #[cfg(not(unix))]
    true
}

fn write_client_config(
    client: AgentClient,
    home: &Path,
    executable: &Path,
) -> Result<(PathBuf, bool)> {
    let path = client.config_path(home);
    let changed = match client {
        AgentClient::Claude | AgentClient::Cursor => write_json_config(&path, executable)?,
        AgentClient::Codex => write_codex_config(&path, executable)?,
    };
    Ok((path, changed))
}

fn write_json_config(path: &Path, executable: &Path) -> Result<bool> {
    let mut root = if path.exists() {
        serde_json::from_slice::<Value>(&fs::read(path)?)
            .with_context(|| format!("{} is not valid JSON", path.display()))?
    } else {
        Value::Object(Map::new())
    };
    let object = root
        .as_object_mut()
        .with_context(|| format!("{} must contain a JSON object", path.display()))?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .with_context(|| format!("{}.mcpServers must be a JSON object", path.display()))?;
    let expected = json!({
        "type": "stdio",
        "command": executable.to_string_lossy(),
        "args": ["--mcp"]
    });
    if servers.get("ig") == Some(&expected) {
        return Ok(false);
    }
    servers.insert("ig".to_string(), expected);
    let mut bytes = serde_json::to_vec_pretty(&root)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)?;
    Ok(true)
}

fn write_codex_config(path: &Path, executable: &Path) -> Result<bool> {
    let source = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let mut document = if source.trim().is_empty() {
        DocumentMut::new()
    } else {
        source
            .parse::<DocumentMut>()
            .with_context(|| format!("{} is not valid TOML", path.display()))?
    };
    if codex_document_matches(&document, executable) {
        return Ok(false);
    }
    if !document.contains_key("mcp_servers") {
        document["mcp_servers"] = Item::Table(Table::new());
    }
    let servers = document["mcp_servers"]
        .as_table_mut()
        .with_context(|| format!("{}.mcp_servers must be a TOML table", path.display()))?;
    let mut server = Table::new();
    server["command"] = value(executable.to_string_lossy().to_string());
    let mut args = Array::new();
    args.push("--mcp");
    server["args"] = Item::Value(TomlValue::Array(args));
    server["enabled"] = value(true);
    servers.insert("ig", Item::Table(server));
    write_atomic(path, document.to_string().as_bytes())?;
    Ok(true)
}

fn config_matches(client: AgentClient, home: &Path, executable: &Path) -> Result<bool> {
    let path = client.config_path(home);
    if !path.exists() {
        return Ok(false);
    }
    match client {
        AgentClient::Claude | AgentClient::Cursor => json_config_matches(&path, executable),
        AgentClient::Codex => {
            let document = fs::read_to_string(&path)?
                .parse::<DocumentMut>()
                .with_context(|| format!("{} is not valid TOML", path.display()))?;
            Ok(codex_document_matches(&document, executable))
        }
    }
}

fn json_config_matches(path: &Path, executable: &Path) -> Result<bool> {
    let root: Value = serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    let Some(server) = root["mcpServers"]["ig"].as_object() else {
        return Ok(false);
    };
    let command = server.get("command").and_then(Value::as_str);
    let args = server.get("args").and_then(Value::as_array);
    Ok(
        command.is_some_and(|command| command_matches(command, executable))
            && args.is_some_and(|args| args.iter().filter_map(Value::as_str).eq(["--mcp"])),
    )
}

fn codex_document_matches(document: &DocumentMut, executable: &Path) -> bool {
    let Some(server) = document
        .get("mcp_servers")
        .and_then(Item::as_table_like)
        .and_then(|servers| servers.get("ig"))
        .and_then(Item::as_table_like)
    else {
        return false;
    };
    let command = server.get("command").and_then(Item::as_str);
    let args = server.get("args").and_then(Item::as_array);
    let enabled = server
        .get("enabled")
        .and_then(Item::as_bool)
        .unwrap_or(true);
    enabled
        && command.is_some_and(|command| command_matches(command, executable))
        && args.is_some_and(|args| args.iter().filter_map(TomlValue::as_str).eq(["--mcp"]))
}

fn command_matches(command: &str, executable: &Path) -> bool {
    let expected = executable
        .canonicalize()
        .unwrap_or_else(|_| executable.to_path_buf());
    let configured = Path::new(command);
    if configured.is_absolute() || configured.components().count() > 1 {
        return configured
            .canonicalize()
            .is_ok_and(|configured| configured == expected);
    }
    find_executable(command)
        .is_some_and(|configured| configured.canonicalize().unwrap_or(configured) == expected)
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.ivygrep-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config"),
        Uuid::new_v4()
    ));
    fs::write(&temporary, contents)?;
    if let Ok(metadata) = fs::metadata(path) {
        fs::set_permissions(&temporary, metadata.permissions())?;
    }
    #[cfg(unix)]
    if !path.exists() {
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            let original = fs::read(path)?;
            fs::remove_file(path)?;
            if let Err(error) = fs::rename(&temporary, path) {
                let _ = fs::write(path, original);
                let _ = fs::remove_file(&temporary);
                return Err(error).context("failed to replace config atomically");
            }
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error).with_context(|| format!("failed to write {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_config_preserves_existing_servers_and_keys() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"theme":"dark","mcpServers":{"other":{"command":"other"}}}"#,
        )
        .unwrap();
        let executable = env::current_exe().unwrap().canonicalize().unwrap();
        assert!(write_json_config(&path, &executable).unwrap());
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["theme"], "dark");
        assert_eq!(value["mcpServers"]["other"]["command"], "other");
        assert_eq!(value["mcpServers"]["ig"]["args"], json!(["--mcp"]));
        assert!(!write_json_config(&path, &executable).unwrap());
    }

    #[test]
    fn codex_config_preserves_comments_and_other_servers() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            "# keep me\nmodel = \"gpt-test\"\n\n[mcp_servers.other]\ncommand = \"other\"\n",
        )
        .unwrap();
        let executable = env::current_exe().unwrap().canonicalize().unwrap();
        assert!(write_codex_config(&path, &executable).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep me"));
        assert!(text.contains("model = \"gpt-test\""));
        assert!(text.contains("[mcp_servers.other]"));
        assert!(text.contains("[mcp_servers.ig]"));
        assert!(!write_codex_config(&path, &executable).unwrap());
    }

    #[test]
    fn codex_config_reenables_disabled_server() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let executable = env::current_exe().unwrap().canonicalize().unwrap();
        fs::write(
            &path,
            format!(
                "[mcp_servers.ig]\ncommand = {:?}\nargs = [\"--mcp\"]\nenabled = false\n",
                executable.to_string_lossy()
            ),
        )
        .unwrap();

        assert!(write_codex_config(&path, &executable).unwrap());
        let document = fs::read_to_string(&path)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert!(codex_document_matches(&document, &executable));
        assert!(!write_codex_config(&path, &executable).unwrap());
    }

    #[test]
    fn malformed_configs_are_not_changed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mcp.json");
        fs::write(&path, "{broken").unwrap();
        let before = fs::read(&path).unwrap();
        assert!(write_json_config(&path, Path::new("/tmp/ig")).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn configured_symlink_matches_running_binary() {
        let temp = tempfile::tempdir().unwrap();
        let executable = env::current_exe().unwrap().canonicalize().unwrap();
        let stable = temp.path().join("ig");
        std::os::unix::fs::symlink(&executable, &stable).unwrap();
        assert!(command_matches(&stable.to_string_lossy(), &stable));
        assert!(command_matches(&stable.to_string_lossy(), &executable));
    }
}
