//! ClientMiddleware trait — per-server middleware.
//!
//! Pattern from MCProxy: before/after hooks around each server operation.
//! Key pattern: `MiddlewareAction::Block` can short-circuit tool calls.

use crate::middleware::MiddlewareAction;
use async_trait::async_trait;
use uuid::Uuid;

/// Trait for middleware that operates on individual server calls.
///
/// Pattern from MCProxy's ClientMiddleware:
/// - Each handler method receives a unique request_id for correlation.
/// - `before_*` can return `Block` to short-circuit the operation.
/// - `after_*` receives the result for inspection/modification.
#[async_trait]
pub trait ClientMiddleware: Send + Sync {
    /// Called before listing tools from a server.
    async fn before_list_tools(
        &self,
        _server_name: &str,
        _request_id: Uuid,
    ) -> MiddlewareAction<()> {
        MiddlewareAction::Continue
    }

    /// Called after listing tools from a server.
    async fn after_list_tools(
        &self,
        _server_name: &str,
        _request_id: Uuid,
        _tool_names: Vec<String>,
    ) -> MiddlewareAction<Vec<String>> {
        MiddlewareAction::Continue
    }

    /// Called before calling a tool on a server.
    async fn before_call_tool(
        &self,
        _server_name: &str,
        _tool_name: &str,
        _arguments: &serde_json::Value,
        _request_id: Uuid,
    ) -> MiddlewareAction<()> {
        MiddlewareAction::Continue
    }

    /// Called after calling a tool on a server.
    async fn after_call_tool(
        &self,
        _server_name: &str,
        _tool_name: &str,
        _request_id: Uuid,
        _result: String,
    ) -> MiddlewareAction<String> {
        MiddlewareAction::Continue
    }

    /// Get the name of this middleware for logging.
    fn name(&self) -> &str;
}
