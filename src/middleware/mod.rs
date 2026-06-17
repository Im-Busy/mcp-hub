//! Middleware system for mcp-hub.
//!
//! Pattern adopted from MCProxy's dual middleware architecture:
//!
//! - **ClientMiddleware**: Operates per-server, intercepts individual
//!   server calls (logging, security filtering, caching).
//!   Pattern: before_* and after_* hooks around each operation.
//!
//! - **ProxyMiddleware**: Operates on aggregated results across all
//!   servers (description enrichment, tool search, deduplication).
//!
//! - **MiddlewareRegistry**: Factory pattern for registering and
//!   creating middleware by type name from configuration.

use crate::config::{MiddlewareConfig, MiddlewareSpec};
use crate::error::HubResult;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

// Re-export submodules
pub mod client;
pub mod proxy_trait;

// Subdirectories
pub mod client_middleware;
pub mod proxy_middleware;

// Re-import for convenience
use client::ClientMiddleware;
use proxy_trait::ProxyMiddleware;

/// Result of a middleware operation.
#[derive(Debug, Clone)]
pub enum MiddlewareAction<T> {
    /// Continue with the original value.
    Continue,
    /// Replace with a modified value.
    Replace(T),
    /// Block the operation with an error message.
    Block(String),
}

/// Manages the loading and lifecycle of middleware chains.
///
/// Pattern from MCProxy: loads middleware from config, applies
/// per-server overrides, constructs ordered chains.
pub struct MiddlewareManager {
    /// Registry of available middleware factories.
    registry: MiddlewareRegistry,

    /// Configuration for middleware.
    config: MiddlewareConfig,
}

impl MiddlewareManager {
    /// Create a new middleware manager from configuration.
    pub fn new(config: MiddlewareConfig) -> Self {
        let registry = MiddlewareRegistry::new();
        Self { registry, config }
    }

    /// Get the client middleware chain for a specific server.
    ///
    /// Resolves per-server overrides — if the server has specific
    /// middleware configured, it completely replaces the default.
    pub fn get_client_middleware_for_server(
        &self,
        server_name: &str,
    ) -> HubResult<Vec<Arc<dyn ClientMiddleware>>> {
        let specs = if let Some(server_specs) = self.config.client.servers.get(server_name) {
            debug!(
                server = %server_name,
                count = server_specs.len(),
                "Using server-specific middleware"
            );
            server_specs.clone()
        } else {
            debug!(
                server = %server_name,
                count = self.config.client.default.len(),
                "Using default middleware"
            );
            self.config.client.default.clone()
        };

        self.build_client_middleware_chain(&specs)
    }

    /// Get the proxy middleware chain.
    pub fn get_proxy_middleware(&self) -> HubResult<Vec<Arc<dyn ProxyMiddleware>>> {
        self.build_proxy_middleware_chain(&self.config.proxy)
    }

    /// Build a client middleware chain from specs.
    fn build_client_middleware_chain(
        &self,
        specs: &[MiddlewareSpec],
    ) -> HubResult<Vec<Arc<dyn ClientMiddleware>>> {
        let mut chain = Vec::new();
        for spec in specs {
            if !spec.enabled {
                debug!(type_name = %spec.r#type, "Skipping disabled middleware");
                continue;
            }
            match self.registry.create_client(&spec.r#type, &spec.config)? {
                Some(mw) => chain.push(mw),
                None => warn!(
                    type_name = %spec.r#type,
                    "Unknown client middleware type, skipping"
                ),
            }
        }
        Ok(chain)
    }

    /// Build a proxy middleware chain from specs.
    fn build_proxy_middleware_chain(
        &self,
        specs: &[MiddlewareSpec],
    ) -> HubResult<Vec<Arc<dyn ProxyMiddleware>>> {
        let mut chain = Vec::new();
        for spec in specs {
            if !spec.enabled {
                continue;
            }
            match self.registry.create_proxy(&spec.r#type, &spec.config)? {
                Some(mw) => chain.push(mw),
                None => warn!(
                    type_name = %spec.r#type,
                    "Unknown proxy middleware type, skipping"
                ),
            }
        }
        Ok(chain)
    }
}

/// Registry of middleware factories.
///
/// Pattern from MCProxy: string-keyed hashmap of factory functions.
/// New middleware types are registered here and instantiated from config.
pub struct MiddlewareRegistry {
    /// Client middleware factories.
    client_factories: HashMap<
        String,
        Box<dyn Fn(&serde_json::Value) -> HubResult<Arc<dyn ClientMiddleware>> + Send + Sync>,
    >,
    /// Proxy middleware factories.
    proxy_factories: HashMap<
        String,
        Box<dyn Fn(&serde_json::Value) -> HubResult<Arc<dyn ProxyMiddleware>> + Send + Sync>,
    >,
}

impl MiddlewareRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        let mut registry = Self {
            client_factories: HashMap::new(),
            proxy_factories: HashMap::new(),
        };

        // Register built-in middleware
        Self::register_builtins(&mut registry);

        registry
    }

    /// Register all built-in middleware types.
    fn register_builtins(registry: &mut Self) {
        // Client middleware
        registry.register_client_factory("logging", |config| {
            let level = config.get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info")
                .to_string();
            Ok(Arc::new(client_middleware::LoggingMiddleware::new(level)))
        });

        registry.register_client_factory("security", |config| {
            let rules = config.get("rules")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| {
                            Some(client_middleware::SecurityRule {
                                name: r.get("name")?.as_str()?.to_string(),
                                pattern: r.get("pattern")?.as_str()?.to_string(),
                                block_message: r.get("block_message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Blocked by security policy")
                                    .to_string(),
                                enabled: r.get("enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            Ok(Arc::new(client_middleware::SecurityMiddleware::new(rules)))
        });

        registry.register_client_factory("tool_filter", |config| {
            let allow = config.get("allow")
                .and_then(|v| v.as_str())
                .map(String::from);
            let disallow = config.get("disallow")
                .and_then(|v| v.as_str())
                .map(String::from);
            Ok(Arc::new(client_middleware::ToolFilterMiddleware::new(allow, disallow)))
        });

        // Proxy middleware
        registry.register_proxy_factory("description_enricher", |config| {
            let suffix = config.get("suffix")
                .and_then(|v| v.as_str())
                .unwrap_or("(via mcp-hub)")
                .to_string();
            Ok(Arc::new(proxy_middleware::DescriptionEnricherMiddleware::new(suffix)))
        });

        registry.register_proxy_factory("tool_search", |config| {
            let cfg: proxy_middleware::tool_search::ToolSearchConfig =
                serde_json::from_value(config.clone()).unwrap_or_default();
            Ok(Arc::new(proxy_middleware::tool_search::ToolSearchMiddleware::new(cfg)))
        });
    }

    /// Register a client middleware factory.
    pub fn register_client_factory<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&serde_json::Value) -> HubResult<Arc<dyn ClientMiddleware>> + Send + Sync + 'static,
    {
        self.client_factories
            .insert(name.to_string(), Box::new(factory));
    }

    /// Register a proxy middleware factory.
    pub fn register_proxy_factory<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&serde_json::Value) -> HubResult<Arc<dyn ProxyMiddleware>> + Send + Sync + 'static,
    {
        self.proxy_factories
            .insert(name.to_string(), Box::new(factory));
    }

    /// Create a client middleware instance by type name.
    pub fn create_client(
        &self,
        type_name: &str,
        config: &serde_json::Value,
    ) -> HubResult<Option<Arc<dyn ClientMiddleware>>> {
        match self.client_factories.get(type_name) {
            Some(factory) => Ok(Some(factory(config)?)),
            None => Ok(None),
        }
    }

    /// Create a proxy middleware instance by type name.
    pub fn create_proxy(
        &self,
        type_name: &str,
        config: &serde_json::Value,
    ) -> HubResult<Option<Arc<dyn ProxyMiddleware>>> {
        match self.proxy_factories.get(type_name) {
            Some(factory) => Ok(Some(factory(config)?)),
            None => Ok(None),
        }
    }
}

impl Default for MiddlewareRegistry {
    fn default() -> Self {
        Self::new()
    }
}
