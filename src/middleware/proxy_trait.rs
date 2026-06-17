//! ProxyMiddleware trait — cross-server middleware.
//!
//! Pattern from MCProxy: operates on the final aggregated results
//! from all connected servers. Used for description enrichment,
//! tool search, deduplication, etc.

use async_trait::async_trait;
use crate::proxy::AggregatedTool;

/// Trait for middleware that operates on aggregated proxy results.
///
/// Pattern from MCProxy's ProxyMiddleware:
/// - Receives mutable references to aggregated collections.
/// - Can add, remove, or modify tools/prompts/resources.
/// - Multiple proxy middleware chain together.
#[async_trait]
pub trait ProxyMiddleware: Send + Sync {
    /// Transform the aggregated tool list.
    async fn on_list_tools(
        &self,
        tools: &mut Vec<AggregatedTool>,
    );

    /// Get the name of this middleware for logging.
    fn name(&self) -> &str;
}
