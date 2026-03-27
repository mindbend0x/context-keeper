use crate::error::Result;
use crate::traits::QueryRewriter;

/// Expands search queries to improve recall when initial results are sparse.
///
/// When a raw query returns few results (below a configurable threshold):
/// 1. LLM rewrites the query into 3–5 semantic variants
/// 2. All variants are embedded and searched in parallel
/// 3. Results are merged and re-ranked via RRF
/// 4. If still insufficient, expands to 2-hop graph neighbors
pub struct QueryExpander {
    /// Minimum result count before expansion is triggered.
    pub threshold: usize,
}

impl QueryExpander {
    pub fn new(threshold: usize) -> Self {
        Self { threshold }
    }

    /// Generate expanded query variants using the rewriter.
    pub async fn expand(
        &self,
        query: &str,
        rewriter: &dyn QueryRewriter,
    ) -> Result<Vec<String>> {
        tracing::info!(query, "Expanding search query");
        let variants = rewriter.rewrite(query).await?;
        tracing::info!(count = variants.len(), "Generated query variants");
        Ok(variants)
    }

    /// Check if the result count is below the expansion threshold.
    pub fn should_expand(&self, result_count: usize) -> bool {
        result_count < self.threshold
    }
}
