//! STDIO transport — connects to MCP servers via child process.
//!
//! Pattern learned from mcp-proxy-tool: spawn child process with piped
//! stdin/stdout/stderr. Process group management for clean shutdown
//! (SIGTERM → timeout → SIGKILL escalation from tool-cli).

use crate::config::ServerConfig;
use crate::error::HubError;
use tracing::{info, warn};

/// Connect to a STDIO-based MCP server. Returns the spawned child process.
pub fn connect_stdio(
    config: &ServerConfig,
    server_name: &str,
) -> Result<tokio::process::Child, HubError> {
    let (command, args, env) = match config {
        ServerConfig::Stdio { command, args, env } => (command, args, env),
        _ => return Err(HubError::Transport(format!(
            "Expected Stdio config for server '{}'", server_name
        ))),
    };

    info!(server = %server_name, command = %command, "Spawning STDIO MCP server");

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args);
    for (k, v) in env { cmd.env(k, v); }

    #[cfg(unix)] { unsafe { cmd.pre_exec(|| { libc::setpgid(0, 0); Ok(()) }); } }

    let child = cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn().map_err(|e| HubError::Connection {
            server: server_name.into(), message: format!("{}", e),
        })?;

    info!(server = %server_name, pid = ?child.id(), "STDIO server started");
    Ok(child)
}

/// Graceful shutdown: SIGTERM → timeout → SIGKILL (from tool-cli).
pub async fn shutdown_stdio_process(
    server_name: &str,
    mut child: tokio::process::Child,
    timeout_secs: u64,
) {
    info!(server = %server_name, "Shutting down STDIO process");
    #[cfg(unix)] {
        if let Some(pid) = child.id() { unsafe { libc::kill(-(pid as i32), libc::SIGTERM); } }
    }
    match tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), child.wait()).await {
        Ok(Ok(s)) => info!(server = %server_name, exit = ?s.code(), "Process exited"),
        Ok(Err(e)) => warn!(server = %server_name, error = %e, "Wait error"),
        Err(_) => { warn!(server = %server_name, "Timeout, force kill"); let _ = child.start_kill(); }
    }
}
