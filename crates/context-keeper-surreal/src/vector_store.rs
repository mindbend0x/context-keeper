use crate::repository::{cosine_similarity, Repository};
use anyhow::Result;
use context_keeper_core::models::Entity;

/// Wraps the Repository's vector search as a convenience layer.
/// In production with HNSW indexes, this would delegate to SurrealDB's
/// native vector search. In embedded mode, it uses brute-force cosine.
pub struct SurrealVectorStore {
    repo: Repository,
}

impl SurrealVectorStore {
    pub fn new(repo: Repository) -> Self {
        Self { repo }
    }

    /// Query for the top-k nearest entity neighbors by embedding similarity.
    pub async fn top_k(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let results = self.repo.search_entities_by_vector(embedding, k).await?;
        Ok(results
            .into_iter()
            .map(|(entity, score)| VectorSearchResult {
                id: entity.id.to_string(),
                score,
                content: entity.summary.clone(),
                entity: Some(entity),
            })
            .collect())
    }
}

/// A single result from vector similarity search.
#[derive(Debug)]
pub struct VectorSearchResult {
    pub id: String,
    pub score: f32,
    pub content: String,
    pub entity: Option<Entity>,
}
