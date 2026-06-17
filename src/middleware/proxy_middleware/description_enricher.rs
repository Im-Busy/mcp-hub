//! Description enricher middleware — adds branding to tool descriptions.
//!
//! Pattern from MCProxy: transparently appends a suffix to every tool's
//! description so clients can identify which tools went through the proxy.

use async_trait::async_trait;
use tracing::info;

use crate::middleware::proxy_trait::ProxyMiddleware;
use crate::proxy::AggregatedTool;

pub struct DescriptionEnricherMiddleware {
    suffix: String,
}

impl DescriptionEnricherMiddleware {
    pub fn new(suffix: String) -> Self {
        Self { suffix }
    }
}

#[async_trait]
impl ProxyMiddleware for DescriptionEnricherMiddleware {
    async fn on_list_tools(&self, tools: &mut Vec<AggregatedTool>) {
        for tool in tools.iter_mut() {
            if !tool.description.contains(&self.suffix) {
                tool.description = format!("{} {}", tool.description, self.suffix);
            }
        }
        info!(
            count = tools.len(),
            suffix = %self.suffix,
            "Enriched tool descriptions"
        );
    }

    fn name(&self) -> &str {
        "description_enricher"
    }
}
