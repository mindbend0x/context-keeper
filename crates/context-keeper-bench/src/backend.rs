use async_trait::async_trait;
use context_keeper_core::traits::{ExtractedEntity, ExtractedRelation};
use serde::Serialize;

// ── Output types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EntityExtractionOutput {
    pub entities: Vec<ExtractedEntity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationExtractionOutput {
    pub entities: Vec<ExtractedEntity>,
    pub relations: Vec<ExtractedRelation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestionOutput {
    pub entity_count: usize,
    pub relation_count: usize,
    pub memory_count: usize,
    pub entities_created: usize,
    pub entities_updated: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchOutput {
    pub result_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryRewriteOutput {
    pub variants: Vec<String>,
}

// ── Backend trait ───────────────────────────────────────────────────────

/// Abstraction over a memory-graph system that can be benchmarked.
///
/// Context Keeper implements this today; future backends (e.g. Graphiti)
/// would add their own implementation so the same scenarios can compare
/// different systems head-to-head.
#[async_trait]
pub trait BenchBackend: Send + Sync {
    fn name(&self) -> &str;

    async fn entity_extraction(&self, text: &str) -> anyhow::Result<EntityExtractionOutput>;

    async fn relation_extraction(&self, text: &str) -> anyhow::Result<RelationExtractionOutput>;

    async fn ingestion(&self, text: &str, source: &str) -> anyhow::Result<IngestionOutput>;

    async fn search(&self, query: &str) -> anyhow::Result<SearchOutput>;

    async fn query_rewrite(&self, query: &str) -> anyhow::Result<QueryRewriteOutput>;

    /// Search the graph and return entity names from the results.
    /// Used by behavioral scenarios to verify expected/unexpected entities.
    async fn search_entity_names(&self, query: &str) -> anyhow::Result<Vec<String>>;

    /// Reset internal state (e.g. drop all entities). Called between behavioral iterations.
    async fn reset(&self) -> anyhow::Result<()>;
}
