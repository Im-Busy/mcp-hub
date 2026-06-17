//! STDIO transport — connects to MCP servers via child process.
//!
//! Pattern adopted from MCProxy: spawn child process with piped stdin/stdout,
//! create rmcp transport from (stdout, stdin) tuple, call handler.serve().
//! Process group management for clean shutdown (SIGTERM → timeout → SIGKILL).

use crate::config::ServerConfig;
use crate::error::{HubError, HubResult};
use rmcp::service::{RoleClient, RunningService, ServiceExt};
use tokio::process::Child;
use tracing::{info, warn};

/// Connect to a STDIO-based MCP server via rmcp.
///
/// Spawns the child process, extracts piped stdout/stdin,
/// creates an rmcp transport, and returns a connected client.
pub async fn connect_stdio(
    config: &ServerConfig,
    server_name: &str,
) -> HubResult<(RunningService<RoleClient, ()>, Child)> {
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

    #[cfg(unix)]
    unsafe { cmd.pre_exec(|| { libc::setpgid(0, 0); Ok(()) }); }

    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| HubError::ProcessSpawn(format!(
            "Failed to spawn '{}' for server '{}': {}", command, server_name, e
        )))?;

    info!(server = %server_name, pid = ?child.id(), "STDIO server process started");

    let stdout = child.stdout.take().expect("child stdout is piped");
    let stdin = child.stdin.take().expect("child stdin is piped");

    // Create rmcp transport from (stdout, stdin) tuple — IntoTransport is auto-implemented
    let transport = (stdout, stdin);
    let handler = ();

    info!(server = %server_name, "Establishing rmcp client connection via stdio");

    match handler.serve(transport).await {
        Ok(client) => {
            info!(server = %server_name, "STDIO MCP connection established");
            Ok((client, child))
        }
        Err(e) => {
            warn!(server = %server_name, error = %e, "Failed to connect to stdio server, killing process");
            let _ = child.kill().await;
            Err(HubError::Connection {
                server: server_name.to_string(),
                message: format!("rmcp serve failed: {}", e),
            })
        }
    }
}

/// Graceful shutdown: SIGTERM → timeout → SIGKILL (from tool-cli pattern).
pub async fn shutdown_stdio_process(
    server_name: &str,
    mut child: Child,
    timeout_secs: u64,
) {
    info!(server = %server_name, "Shutting down STDIO process");

    #[cfg(unix)]
    unsafe {
        if let Some(pid) = child.id() {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }
    }

    match tokio::time::timeout(
        tokio::time::Duration::from_secs(timeout_secs),
        child.wait(),
    ).await {
        Ok(Ok(status)) => info!(server = %server_name, exit = ?status.code(), "Process exited"),
        Ok(Err(e)) => warn!(server = %server_name, error = %e, "Wait error"),
        Err(_) => {
            warn!(server = %server_name, "Timeout — force kill");
            let _ = child.start_kill();
        }
    }
}
