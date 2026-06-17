//! Built-in client middleware implementations.
//!
//! Pattern from MCProxy: each middleware type is a struct implementing
//! the ClientMiddleware trait. Configuration is passed via JSON config.

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::middleware::client::ClientMiddleware;
use crate::middleware::MiddlewareAction;

// Re-export for registry
pub use logging::LoggingMiddleware;
pub use security::{SecurityMiddleware, SecurityRule};
pub use tool_filter::ToolFilterMiddleware;

/// Logging middleware — logs all operations with timing.
pub mod logging {
    use super::*;

    pub struct LoggingMiddleware {
        #[allow(dead_code)]
        level: String,
    }

    impl LoggingMiddleware {
        pub fn new(level: String) -> Self {
            Self { level }
        }
    }

    #[async_trait]
    impl ClientMiddleware for LoggingMiddleware {
        async fn before_list_tools(
            &self,
            server_name: &str,
            request_id: Uuid,
        ) -> MiddlewareAction<()> {
            info!(
                server = %server_name,
                request_id = %request_id,
                "Listing tools from server"
            );
            MiddlewareAction::Continue
        }

        async fn after_list_tools(
            &self,
            server_name: &str,
            request_id: Uuid,
            tool_names: Vec<String>,
        ) -> MiddlewareAction<Vec<String>> {
            info!(
                server = %server_name,
                request_id = %request_id,
                count = tool_names.len(),
                "Listed tools from server"
            );
            MiddlewareAction::Continue
        }

        async fn before_call_tool(
            &self,
            server_name: &str,
            tool_name: &str,
            _arguments: &serde_json::Value,
            request_id: Uuid,
        ) -> MiddlewareAction<()> {
            info!(
                server = %server_name,
                tool = %tool_name,
                request_id = %request_id,
                "Calling tool"
            );
            MiddlewareAction::Continue
        }

        async fn after_call_tool(
            &self,
            server_name: &str,
            tool_name: &str,
            request_id: Uuid,
            result: String,
        ) -> MiddlewareAction<String> {
            let preview = if result.len() > 200 {
                format!("{}...", &result[..200])
            } else {
                result.clone()
            };
            info!(
                server = %server_name,
                tool = %tool_name,
                request_id = %request_id,
                result_preview = %preview,
                "Tool call completed"
            );
            MiddlewareAction::Continue
        }

        fn name(&self) -> &str {
            "logging"
        }
    }
}

/// Security middleware — inspects tool call inputs against security rules.
pub mod security {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct SecurityRule {
        pub name: String,
        pub pattern: String,
        pub block_message: String,
        pub enabled: bool,
    }

    pub struct SecurityMiddleware {
        rules: Vec<(SecurityRule, Regex)>,
    }

    impl SecurityMiddleware {
        pub fn new(rules: Vec<SecurityRule>) -> Self {
            let compiled = rules
                .into_iter()
                .filter(|r| r.enabled)
                .filter_map(|r| {
                    match Regex::new(&r.pattern) {
                        Ok(re) => Some((r, re)),
                        Err(e) => {
                            error!(rule = %r.name, error = %e, "Invalid regex in security rule");
                            None
                        }
                    }
                })
                .collect();
            Self { rules: compiled }
        }
    }

    #[async_trait]
    impl ClientMiddleware for SecurityMiddleware {
        async fn before_call_tool(
            &self,
            server_name: &str,
            tool_name: &str,
            arguments: &serde_json::Value,
            request_id: Uuid,
        ) -> MiddlewareAction<()> {
            // Build searchable content from tool call
            let content = format!(
                "tool:{} args:{}",
                tool_name,
                serde_json::to_string(arguments).unwrap_or_default()
            );

            for (rule, regex) in &self.rules {
                if regex.is_match(&content) {
                    warn!(
                        server = %server_name,
                        rule = %rule.name,
                        request_id = %request_id,
                        "Security rule triggered"
                    );
                    return MiddlewareAction::Block(rule.block_message.clone());
                }
            }

            MiddlewareAction::Continue
        }

        fn name(&self) -> &str {
            "security"
        }
    }
}

/// Tool filter middleware — filters tools with regex patterns.
pub mod tool_filter {
    use super::*;

    pub struct ToolFilterMiddleware {
        allow_regex: Option<Regex>,
        disallow_regex: Option<Regex>,
    }

    impl ToolFilterMiddleware {
        pub fn new(allow: Option<String>, disallow: Option<String>) -> Self {
            let allow_regex = allow.and_then(|p| Regex::new(&p).ok());
            let disallow_regex = disallow.and_then(|p| Regex::new(&p).ok());

            if let Some(ref re) = allow_regex {
                info!(pattern = %re, "Tool filter allow pattern compiled");
            }
            if let Some(ref re) = disallow_regex {
                info!(pattern = %re, "Tool filter disallow pattern compiled");
            }

            Self {
                allow_regex,
                disallow_regex,
            }
        }
    }

    #[async_trait]
    impl ClientMiddleware for ToolFilterMiddleware {
        async fn after_list_tools(
            &self,
            server_name: &str,
            _request_id: Uuid,
            tool_names: Vec<String>,
        ) -> MiddlewareAction<Vec<String>> {
            let original_count = tool_names.len();
            let filtered: Vec<String> = tool_names
                .into_iter()
                .filter(|name| {
                    if let Some(ref re) = self.allow_regex {
                        return re.is_match(name);
                    }
                    if let Some(ref re) = self.disallow_regex {
                        if re.is_match(name) {
                            debug!(server = %server_name, tool = %name, "Tool filtered (disallow match)");
                            return false;
                        }
                    }
                    true
                })
                .collect();

            let removed = original_count - filtered.len();
            if removed > 0 {
                info!(server = %server_name, removed = removed, "Filtered tools");
            }

            MiddlewareAction::Replace(filtered)
        }

        fn name(&self) -> &str {
            "tool_filter"
        }
    }
}

// Alias re-exports consumed by middleware registry
#[allow(unused_imports)]
use logging::LoggingMiddleware as _LoggingMw;
#[allow(unused_imports)]
use security::SecurityMiddleware as _SecurityMw;
#[allow(unused_imports)]
use tool_filter::ToolFilterMiddleware as _ToolFilterMw;
