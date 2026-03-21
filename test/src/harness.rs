use anyhow::Result;
use chrono::Utc;
use context_keeper_core::{
    ingestion,
    ingestion::IngestionResult,
    models::*,
    search::fuse_rrf,
    traits::*,
};
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use uuid::Uuid;

pub const EMBED_DIM: usize = 8;

pub struct TestEnv {
    pub repo: Repository,
    pub embedder: MockEmbedder,
    pub entity_extractor: MockEntityExtractor,
    pub relation_extractor: MockRelationExtractor,
    pub query_rewriter: MockQueryRewriter,
}

impl TestEnv {
    pub async fn new() -> Result<Self> {
        let config = SurrealConfig {
            embedding_dimensions: EMBED_DIM,
            ..SurrealConfig::default()
        };
        let db = connect_memory(&config).await?;
        apply_schema(&db, &config).await?;
        Ok(Self {
            repo: Repository::new(db),
            embedder: MockEmbedder::new(EMBED_DIM),
            entity_extractor: MockEntityExtractor,
            relation_extractor: MockRelationExtractor,
            query_rewriter: MockQueryRewriter,
        })
    }

    pub async fn ingest_text(&self, text: &str, source: &str) -> Result<IngestionResult> {
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            created_at: Utc::now(),
        };

        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
        )
        .await?;

        self.repo.create_episode(&episode).await?;
        for entity in &result.entities {
            self.repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            self.repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            self.repo.create_memory(memory).await?;
        }

        Ok(result)
    }

    pub async fn ingest_batch(&self, texts: &[(&str, &str)]) -> Result<Vec<IngestionResult>> {
        let mut results = Vec::with_capacity(texts.len());
        for (text, source) in texts {
            results.push(self.ingest_text(text, source).await?);
        }
        Ok(results)
    }

    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = self
            .repo
            .search_entities_by_vector(&query_embedding, limit)
            .await?;

        let keyword_results = self
            .repo
            .search_entities_by_keyword(query)
            .await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(fused.into_iter().take(limit).collect())
    }
}
