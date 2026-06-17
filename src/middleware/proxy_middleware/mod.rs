//! Built-in proxy middleware implementations.
//!
//! Pattern from MCProxy: operates on aggregated tool lists across
//! all connected servers. Used for description enrichment, tool search,
//! and deduplication.

pub mod description_enricher;
pub mod tool_search;

// Re-export for registry
pub use description_enricher::DescriptionEnricherMiddleware;
pub use tool_search::{ToolSearchConfig, ToolSearchMiddleware};
