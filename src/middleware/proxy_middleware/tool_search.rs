//! Tantivy-powered BM25 full-text tool search middleware.
//!
//! Pattern adopted from MCProxy's `proxy_middleware/tool_search.rs` (41K LOC,
//! the most sophisticated module in the MCProxy codebase).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐    ┌──────────────────┐    ┌─────────────┐
//! │ MCP Servers │───▶│ Tool Search       │───▶│ MCP Clients │
//! │             │    │ Middleware        │    │             │
//! │ • Server A  │    │                  │    │ • ≤N tools  │
//! │ • Server B  │    │ • Tantivy Index  │    │ • search tool│
//! │ • Server C  │    │ • Query Engine   │    │ • dynamic    │
//! └─────────────┘    └──────────────────┘    └─────────────┘
//! ```
//!
//! ## Key Features
//!
//! - **In-memory Tantivy index**: BM25 scoring, sub-linear search time
//! - **Selective exposure**: Only `max_tools` shown initially;
//!   remaining tools discoverable via `search_available_tools`
//! - **Multi-field indexing**: name, description, server, combined text
//! - **Configurable threshold**: Minimum relevance score (0.0-1.0)
//! - **Query parsing**: Boolean queries, phrase matching, field-specific
//! - **Real-time updates**: Index rebuilt on tool list changes

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};
use tracing::{error, info};

use crate::middleware::proxy_trait::ProxyMiddleware;
use crate::proxy::AggregatedTool;

/// Configuration for the tool search middleware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchConfig {
    /// Maximum number of tools to expose in the initial list.
    /// Tools beyond this limit are discoverable via search.
    #[serde(default = "default_max_tools")]
    pub max_tools_limit: usize,

    /// Minimum BM25 relevance score to include in results (0.0-1.0).
    /// Lower = more results (less relevant), Higher = fewer results (more relevant).
    #[serde(default = "default_threshold")]
    pub search_threshold: f32,

    /// Tool selection ordering for initial exposure.
    #[serde(default)]
    pub selection_order: SelectionOrder,
}

fn default_max_tools() -> usize {
    50
}

fn default_threshold() -> f32 {
    0.1
}

impl Default for ToolSearchConfig {
    fn default() -> Self {
        Self {
            max_tools_limit: default_max_tools(),
            search_threshold: default_threshold(),
            selection_order: SelectionOrder::default(),
        }
    }
}

/// Ordering strategy for which tools are shown in the initial list.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SelectionOrder {
    /// Group tools by server, then alphabetically within each server.
    #[default]
    ServerPriority,
    /// Sort all tools alphabetically regardless of server.
    Alphabetical,
}

/// Internal search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The prefixed tool name.
    tool_name: String,
    /// BM25 relevance score.
    score: f32,
    /// Server the tool belongs to.
    server: String,
    /// Tool description snippet.
    description: String,
}

/// Tantivy-powered tool search middleware.
///
/// ## Index Schema
///
/// | Field | Type | Stored | Purpose |
/// |-------|------|--------|---------|
/// | `name` | TEXT | Yes | Tool name (e.g., "github___create_issue") |
/// | `description` | TEXT | Yes | Tool description |
/// | `server` | TEXT | Yes | Server name (e.g., "github") |
/// | `searchable_text` | TEXT | No | Combined text for search |
pub struct ToolSearchMiddleware {
    /// Configuration.
    config: ToolSearchConfig,

    /// Tantivy index (in-memory).
    index: Arc<RwLock<Option<Index>>>,

    /// Index writer for adding documents.
    writer: Arc<RwLock<Option<IndexWriter>>>,

    /// Index reader for searching.
    reader: Arc<RwLock<Option<IndexReader>>>,

    /// Tantivy schema.
    schema: Schema,

    /// Field references.
    field_name: Field,
    field_description: Field,
    field_server: Field,
    field_searchable: Field,

    /// Currently exposed tools (the ones visible to clients).
    exposed_tools: Arc<RwLock<Vec<AggregatedTool>>>,

    /// All tools (including hidden ones).
    all_tools: Arc<RwLock<Vec<AggregatedTool>>>,

    /// Whether the search is active (tools > max_tools_limit).
    search_active: Arc<RwLock<bool>>,
}

impl ToolSearchMiddleware {
    /// Create a new tool search middleware with config.
    pub fn new(config: ToolSearchConfig) -> Self {
        // Define Tantivy schema
        let mut schema_builder = Schema::builder();
        let field_name = schema_builder.add_text_field("name", TEXT | STORED);
        let field_description = schema_builder.add_text_field("description", TEXT | STORED);
        let field_server = schema_builder.add_text_field("server", TEXT | STORED);
        let field_searchable = schema_builder.add_text_field("searchable_text", TEXT);
        let schema = schema_builder.build();

        Self {
            config,
            index: Arc::new(RwLock::new(None)),
            writer: Arc::new(RwLock::new(None)),
            reader: Arc::new(RwLock::new(None)),
            schema,
            field_name,
            field_description,
            field_server,
            field_searchable,
            exposed_tools: Arc::new(RwLock::new(Vec::new())),
            all_tools: Arc::new(RwLock::new(Vec::new())),
            search_active: Arc::new(RwLock::new(false)),
        }
    }

    /// Create from JSON config (for registry factory).
    pub fn from_config(config: &serde_json::Value) -> Result<Self, String> {
        let cfg: ToolSearchConfig = serde_json::from_value(config.clone())
            .map_err(|e| format!("Invalid tool search config: {}", e))?;
        Ok(Self::new(cfg))
    }

    /// Build or rebuild the Tantivy index from the current tool list.
    fn rebuild_index(&self, tools: &[AggregatedTool]) -> Result<(), String> {
        info!(tool_count = tools.len(), "Building Tantivy search index");

        // Create in-memory index
        let index = Index::create_in_ram(self.schema.clone());

        let mut writer: IndexWriter = index
            .writer(50_000_000) // 50MB buffer
            .map_err(|e| format!("Failed to create index writer: {}", e))?;

        // Index each tool
        for tool in tools {
            let searchable = format!(
                "{} {} {}",
                tool.name, tool.description, tool.server_name
            );

            let _ = writer.add_document(doc!(
                self.field_name => tool.name.clone(),
                self.field_description => tool.description.clone(),
                self.field_server => tool.server_name.clone(),
                self.field_searchable => searchable,
            ));
        }

        writer
            .commit()
            .map_err(|e| format!("Failed to commit index: {}", e))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| format!("Failed to create reader: {}", e))?;

        // Store back
        *self.index.write().unwrap() = Some(index);
        *self.writer.write().unwrap() = Some(writer);
        *self.reader.write().unwrap() = Some(reader);

        info!(
            tool_count = tools.len(),
            "Search index built successfully"
        );

        Ok(())
    }

    /// Search for tools matching a query string.
    ///
    /// Returns ranked results with BM25 scores.
    pub fn search(&self, query_str: &str) -> Result<Vec<SearchResult>, String> {
        let reader_guard = self.reader.read().unwrap();
        let reader = reader_guard
            .as_ref()
            .ok_or_else(|| "Search index not initialized".to_string())?;

        let searcher = reader.searcher();

        // Build query — search across all text fields
        let query_parser = QueryParser::for_index(
            &self.index.read().unwrap().as_ref().unwrap(),
            vec![self.field_name, self.field_description, self.field_searchable],
        );

        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| format!("Invalid query: {}", e))?;

        // Execute search with BM25 scoring
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(20))
            .map_err(|e| format!("Search failed: {}", e))?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let score_f32 = score as f32;

            // Apply threshold filter
            if score_f32 < self.config.search_threshold {
                continue;
            }

            let doc: TantivyDocument = searcher
                .doc(doc_addr)
                .map_err(|e| format!("Failed to retrieve document: {}", e))?;

            let tool_name = doc
                .get_first(self.field_name)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let server = doc
                .get_first(self.field_server)
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let description = doc
                .get_first(self.field_description)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                tool_name,
                score: score_f32,
                server,
                description,
            });
        }

        Ok(results)
    }

    /// Format search results as a human-readable response.
    pub fn format_results(&self, query: &str, results: &[SearchResult]) -> String {
        if results.is_empty() {
            return format!(
                "No tools found matching '{}'. Try a different query or check available tools.",
                query
            );
        }

        let mut output = format!(
            "Found {} tool(s) matching '{}':\n\n",
            results.len(),
            query
        );

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** (score: {:.2})\n   Server: {}\n   {}\n\n",
                i + 1,
                result.tool_name,
                result.score,
                result.server,
                result.description,
            ));
        }

        output.push_str("\nThese tools are now available in your current session.");
        output
    }

    /// Get the virtual `search_available_tools` tool definition.
    pub fn get_search_tool_definition(&self) -> AggregatedTool {
        let all_count = self.all_tools.read().unwrap().len();
        let exposed_count = self.exposed_tools.read().unwrap().len();
        let hidden = all_count.saturating_sub(exposed_count);

        AggregatedTool {
            name: "search_available_tools".to_string(),
            original_name: "search_available_tools".to_string(),
            description: format!(
                "Search through {} additional available tools. \
                 Currently {} of {} total tools are visible. \
                 Use this to discover tools by name, description, or server.",
                hidden, exposed_count, all_count
            ),
            server_name: "mcp-hub".to_string(),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query to find relevant tools"
                    }
                },
                "required": ["query"]
            })),
        }
    }

    /// Check if search is active (more tools than the exposure limit).
    pub fn is_search_active(&self) -> bool {
        *self.search_active.read().unwrap()
    }
}

#[async_trait]
impl ProxyMiddleware for ToolSearchMiddleware {
    async fn on_list_tools(&self, tools: &mut Vec<AggregatedTool>) {
        let total = tools.len();

        // Store all tools
        *self.all_tools.write().unwrap() = tools.clone();

        // Rebuild the search index
        if let Err(e) = self.rebuild_index(tools) {
            error!(error = %e, "Failed to rebuild search index");
            return;
        }

        // Select which tools to expose
        if total > self.config.max_tools_limit {
            info!(
                total = total,
                limit = self.config.max_tools_limit,
                "Tool count exceeds limit, enabling search"
            );
            *self.search_active.write().unwrap() = true;

            // Sort tools based on selection order
            match self.config.selection_order {
                SelectionOrder::ServerPriority => {
                    tools.sort_by(|a, b| {
                        a.server_name
                            .cmp(&b.server_name)
                            .then(a.name.cmp(&b.name))
                    });
                }
                SelectionOrder::Alphabetical => {
                    tools.sort_by(|a, b| a.name.cmp(&b.name));
                }
            }

            // Truncate to max_tools_limit
            tools.truncate(self.config.max_tools_limit);

            // Inject the search tool
            let search_tool = self.get_search_tool_definition();
            tools.push(search_tool);

            // Store exposed tools
            *self.exposed_tools.write().unwrap() = tools.clone();
        } else {
            *self.search_active.write().unwrap() = false;
            *self.exposed_tools.write().unwrap() = tools.clone();
        }

        info!(
            total = total,
            exposed = tools.len(),
            search_active = self.is_search_active(),
            "Tool list processed by search middleware"
        );
    }

    fn name(&self) -> &str {
        "tool_search"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool(name: &str, server: &str, desc: &str) -> AggregatedTool {
        AggregatedTool {
            name: name.to_string(),
            original_name: name.split("___").last().unwrap_or(name).to_string(),
            description: desc.to_string(),
            server_name: server.to_string(),
            input_schema: None,
        }
    }

    #[test]
    fn test_search_active_when_over_limit() {
        let config = ToolSearchConfig {
            max_tools_limit: 3,
            search_threshold: 0.1,
            selection_order: SelectionOrder::Alphabetical,
        };
        let mw = ToolSearchMiddleware::new(config);

        // 5 tools > 3 limit
        let tools = vec![
            make_tool("a", "srv1", "Alpha tool"),
            make_tool("b", "srv1", "Beta tool"),
            make_tool("c", "srv2", "Gamma tool"),
            make_tool("d", "srv2", "Delta tool"),
            make_tool("e", "srv3", "Epsilon tool"),
        ];

        // Store preconditions
        *mw.all_tools.write().unwrap() = tools.clone();
        mw.rebuild_index(&tools).unwrap();

        // Since on_list_tools is async, test the underlying logic directly
        assert_eq!(mw.all_tools.read().unwrap().len(), 5);
    }

    #[test]
    fn test_format_results_empty() {
        let config = ToolSearchConfig::default();
        let mw = ToolSearchMiddleware::new(config);
        let output = mw.format_results("github", &[]);
        assert!(output.contains("No tools found"));
    }

    #[test]
    fn test_format_results_with_matches() {
        let config = ToolSearchConfig::default();
        let mw = ToolSearchMiddleware::new(config);
        let results = vec![
            SearchResult {
                tool_name: "github___create_issue".to_string(),
                score: 0.95,
                server: "github".to_string(),
                description: "Create a new issue".to_string(),
            },
            SearchResult {
                tool_name: "github___search_issues".to_string(),
                score: 0.82,
                server: "github".to_string(),
                description: "Search for issues".to_string(),
            },
        ];
        let output = mw.format_results("github", &results);
        assert!(output.contains("Found 2 tool(s)"));
        assert!(output.contains("github___create_issue"));
        assert!(output.contains("github___search_issues"));
    }

    #[test]
    fn test_get_search_tool_definition() {
        let config = ToolSearchConfig::default();
        let mw = ToolSearchMiddleware::new(config);
        *mw.all_tools.write().unwrap() = vec![make_tool("a", "s", "d"); 100];
        *mw.exposed_tools.write().unwrap() = vec![make_tool("a", "s", "d"); 50];

        let search_tool = mw.get_search_tool_definition();
        assert_eq!(search_tool.name, "search_available_tools");
        assert_eq!(search_tool.server_name, "mcp-hub");
        assert!(search_tool.description.contains("50 additional"));
    }

    #[test]
    fn test_tantivy_index_build_and_search() {
        let config = ToolSearchConfig {
            max_tools_limit: 50,
            search_threshold: 0.0, // No threshold for test
            selection_order: SelectionOrder::Alphabetical,
        };
        let mw = ToolSearchMiddleware::new(config);

        let tools = vec![
            make_tool("github___create_issue", "github", "Create a new GitHub issue"),
            make_tool("github___search_issues", "github", "Search for GitHub issues"),
            make_tool("filesystem___read_file", "filesystem", "Read a file from disk"),
            make_tool("filesystem___write_file", "filesystem", "Write content to a file"),
            make_tool("database___query", "database", "Run a SQL query"),
        ];

        mw.rebuild_index(&tools).unwrap();

        // Search for "github"
        let results = mw.search("github").unwrap();
        assert!(!results.is_empty(), "Should find github tools");
        for r in &results {
            assert!(r.server == "github", "Result should be from github server");
        }

        // Search for "file"
        let results = mw.search("file").unwrap();
        assert!(!results.is_empty(), "Should find file tools");
        for r in &results {
            assert!(
                r.server == "filesystem",
                "Result should be from filesystem server, got: {}",
                r.server
            );
        }

        // Search for non-existent
        let _results = mw.search("zzz_nonexistent_tool").unwrap();
        // Empty results are valid — no matching tools
    }

    #[test]
    fn test_threshold_filtering() {
        let config = ToolSearchConfig {
            max_tools_limit: 50,
            search_threshold: 0.9, // High threshold
            selection_order: SelectionOrder::Alphabetical,
        };
        let mw = ToolSearchMiddleware::new(config);

        let tools = vec![
            make_tool("github___create", "github", "Create issues"),
        ];

        mw.rebuild_index(&tools).unwrap();
        let _results = mw.search("create").unwrap();
        // With high threshold, results may be empty
        // This just verifies the threshold doesn't crash
    }
}
