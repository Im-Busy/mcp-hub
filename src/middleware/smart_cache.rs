//! Smart cache middleware — SHA-256 content-addressable caching for MCP tool calls.
//!
//! Pattern from pluggedin-mcp-proxy: cache keyed by content hash of
//! (server, tool, arguments). Returns cached results instantly when available,
//! logs intent to refresh in background.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use crate::middleware::client::ClientMiddleware;
use crate::middleware::MiddlewareAction;

/// A content-addressable cache entry storing a tool call result.
#[derive(Clone)]
#[allow(dead_code)]
struct CacheEntry {
    result: String,
    inserted_at: Instant,
}

/// Smart cache middleware that uses SHA-256 content hashing for cache keys.
///
/// On each tool call:
/// 1. `before_call_tool` precomputes the cache key from (server, tool, args)
///    and stashes it keyed by `request_id`.
/// 2. `after_call_tool` retrieves the key, checks the cache:
///    - **Hit**: returns `Replace(cached_result)`, spawns background refresh intent.
///    - **Miss**: stores the fresh result, returns `Continue`.
pub struct SmartCacheMiddleware {
    /// Content-addressable cache: SHA-256 hex → cached result.
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Per-request stash: request_id → precomputed cache key.
    pending: Arc<RwLock<HashMap<Uuid, String>>>,
}

impl SmartCacheMiddleware {
    /// Create a new empty smart cache middleware.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Compute a deterministic SHA-256 cache key from tool call parameters.
    ///
    /// The key is the hex digest of `server_name || "||" || tool_name || "||" || serialized_args`.
    /// Different servers, tools, or arguments produce different keys.
    fn cache_key(tool_name: &str, server_name: &str, args: &serde_json::Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(server_name.as_bytes());
        hasher.update(b"||");
        hasher.update(tool_name.as_bytes());
        hasher.update(b"||");
        hasher.update(serde_json::to_string(args).unwrap_or_default().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for SmartCacheMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ClientMiddleware for SmartCacheMiddleware {
    async fn before_call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        request_id: Uuid,
    ) -> MiddlewareAction<()> {
        let key = Self::cache_key(tool_name, server_name, arguments);
        debug!(
            server = %server_name,
            tool = %tool_name,
            request_id = %request_id,
            cache_key = %&key[..8],
            "Precomputed cache key"
        );
        self.pending.write().await.insert(request_id, key);
        MiddlewareAction::Continue
    }

    async fn after_call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        request_id: Uuid,
        result: String,
    ) -> MiddlewareAction<String> {
        // Retrieve the precomputed cache key from before_call_tool
        let cache_key = match self.pending.write().await.remove(&request_id) {
            Some(key) => key,
            None => {
                debug!(
                    server = %server_name,
                    tool = %tool_name,
                    request_id = %request_id,
                    "No pending cache key found, passing through"
                );
                return MiddlewareAction::Continue;
            }
        };

        // Check cache
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(&cache_key) {
            let cached = entry.result.clone();
            drop(cache);

            info!(
                server = %server_name,
                tool = %tool_name,
                request_id = %request_id,
                cache_key = %&cache_key[..8],
                "Cache hit — returning cached result"
            );

            // Spawn background refresh task (log intent, actual re-execution deferred)
            let key = cache_key.clone();
            let srv = server_name.to_string();
            let tool = tool_name.to_string();
            tokio::spawn(async move {
                debug!(
                    server = %srv,
                    tool = %tool,
                    cache_key = %&key[..8],
                    "Background refresh intent logged (re-execution deferred for Phase 2)"
                );
            });

            MiddlewareAction::Replace(cached)
        } else {
            drop(cache);

            debug!(
                server = %server_name,
                tool = %tool_name,
                request_id = %request_id,
                cache_key = %&cache_key[..8],
                "Cache miss — storing result"
            );

            self.cache.write().await.insert(
                cache_key,
                CacheEntry {
                    result: result.clone(),
                    inserted_at: Instant::now(),
                },
            );

            MiddlewareAction::Continue
        }
    }

    fn name(&self) -> &str {
        "smart_cache"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_key_determinism() {
        let args = json!({"query": "hello"});
        let key1 = SmartCacheMiddleware::cache_key("search", "test_server", &args);
        let key2 = SmartCacheMiddleware::cache_key("search", "test_server", &args);
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 64); // SHA-256 hex is 64 characters
    }

    #[test]
    fn test_cache_key_different() {
        let args = json!({"query": "hello"});
        let key1 = SmartCacheMiddleware::cache_key("search", "test_server", &args);
        let key2 = SmartCacheMiddleware::cache_key("fetch", "test_server", &args);
        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_cache_hit_returns_cached() {
        let mw = SmartCacheMiddleware::new();
        let args = json!({"x": 1});

        // First call: populate the cache
        let id1 = Uuid::new_v4();
        mw.before_call_tool("srv", "tool", &args, id1).await;
        let action1 = mw
            .after_call_tool("srv", "tool", id1, "first_result".to_string())
            .await;
        assert!(matches!(action1, MiddlewareAction::Continue));

        // Second call with same args: should return cached result
        let id2 = Uuid::new_v4();
        mw.before_call_tool("srv", "tool", &args, id2).await;
        let action2 = mw
            .after_call_tool("srv", "tool", id2, "second_result".to_string())
            .await;

        match action2 {
            MiddlewareAction::Replace(cached) => {
                assert_eq!(cached, "first_result");
            }
            other => panic!("Expected Replace with cached value, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cache_miss_stores_result() {
        let mw = SmartCacheMiddleware::new();
        let request_id = Uuid::new_v4();

        mw.before_call_tool("srv", "tool", &json!({"x": 1}), request_id)
            .await;

        let action = mw
            .after_call_tool("srv", "tool", request_id, "result_value".to_string())
            .await;

        // Should continue (pass through the result)
        assert!(matches!(action, MiddlewareAction::Continue));

        // Verify the result was stored in the cache
        let cache = mw.cache.read().await;
        assert_eq!(cache.len(), 1);
    }
}
