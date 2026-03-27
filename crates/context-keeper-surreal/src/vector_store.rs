use crate::repository::Repository;
use context_keeper_core::error::Result;
use context_keeper_core::models::{Entity, Memory};

/// Thin wrapper over the Repository's HNSW-backed vector search.
///
/// The repository now delegates cosine similarity to SurrealDB's
/// `vector::similarity::cosine` function backed by an HNSW index,
/// so this layer is purely a convenience API.
pub struct SurrealVectorStore {
    repo: Repository,
}

impl SurrealVectorStore {
    pub fn new(repo: Repository) -> Self {
        Self { repo }
    }

    /// Top-k nearest entity neighbors by embedding similarity (HNSW-backed).
    pub async fn top_k_entities(
        &self,
        embedding: &[f64],
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let results = self.repo.search_entities_by_vector(embedding, k, None, None).await?;
        Ok(results
            .into_iter()
            .map(|(entity, score)| VectorSearchResult {
                id: entity.id.to_string(),
                score,
                content: entity.summary.clone(),
                entity: Some(entity),
                memory: None,
            })
            .collect())
    }

    /// Top-k nearest memory neighbors by embedding similarity (HNSW-backed).
    pub async fn top_k_memories(
        &self,
        embedding: &[f64],
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let results = self.repo.search_memories_by_vector(embedding, k, None).await?;
        Ok(results
            .into_iter()
            .map(|(memory, score)| VectorSearchResult {
                id: memory.id.to_string(),
                score,
                content: memory.content.clone(),
                entity: None,
                memory: Some(memory),
            })
            .collect())
    }

    /// Convenience: delegates to `top_k_entities` (backwards compatibility).
    pub async fn top_k(
        &self,
        embedding: &[f64],
        k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        self.top_k_entities(embedding, k).await
    }
}

/// A single result from vector similarity search.
#[derive(Debug)]
pub struct VectorSearchResult {
    pub id: String,
    pub score: f64,
    pub content: String,
    pub entity: Option<Entity>,
    pub memory: Option<Memory>,
}
