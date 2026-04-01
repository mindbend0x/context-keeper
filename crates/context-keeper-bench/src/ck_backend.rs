use async_trait::async_trait;
use chrono::Utc;
use context_keeper_core::ingestion;
use context_keeper_core::models::Episode;
use context_keeper_core::search::fuse_rrf;
use context_keeper_core::traits::{Embedder, EntityExtractor, EntityResolver, RelationExtractor};
use context_keeper_rig::embeddings::RigEmbedder;
use context_keeper_rig::extraction::{RigEntityExtractor, RigRelationExtractor};
use context_keeper_rig::rewriting::RigQueryRewriter;
use context_keeper_surreal::{apply_schema, connect_memory, Repository, SurrealConfig};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::backend::{
    BenchBackend, EntityExtractionOutput, IngestionOutput, QueryRewriteOutput,
    RelationExtractionOutput, SearchOutput,
};
use crate::config::ProviderConfig;

/// Context Keeper backend that delegates to real Rig-based extractors.
///
/// When `init_storage()` is called, an in-memory SurrealDB is spun up so that
/// `ingestion` persists to the graph and `search` runs actual hybrid search.
pub struct ContextKeeperBackend {
    provider_name: String,
    entity_extractor: RigEntityExtractor,
    relation_extractor: RigRelationExtractor,
    embedder: RigEmbedder,
    query_rewriter: RigQueryRewriter,
    repo: Mutex<Option<Repository>>,
    embedding_dims: usize,
}

impl ContextKeeperBackend {
    pub fn from_provider(provider: &ProviderConfig) -> Self {
        let entity_extractor = RigEntityExtractor::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );
        let relation_extractor = RigRelationExtractor::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );
        let embedder = RigEmbedder::new(
            &provider.api_url,
            &provider.api_key,
            &provider.embedding_model,
            provider.embedding_dims,
        );
        let query_rewriter = RigQueryRewriter::new(
            &provider.api_url,
            &provider.api_key,
            &provider.extraction_model,
        );

        Self {
            provider_name: provider.name.clone(),
            entity_extractor,
            relation_extractor,
            embedder,
            query_rewriter,
            repo: Mutex::new(None),
            embedding_dims: provider.embedding_dims,
        }
    }

    async fn ensure_repo(&self) -> anyhow::Result<()> {
        let mut guard = self.repo.lock().await;
        if guard.is_none() {
            let config = SurrealConfig {
                embedding_dimensions: self.embedding_dims,
                ..SurrealConfig::default()
            };
            let db = connect_memory(&config).await?;
            apply_schema(&db, &config).await?;
            *guard = Some(Repository::new(db));
        }
        Ok(())
    }
}

#[async_trait]
impl BenchBackend for ContextKeeperBackend {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn entity_extraction(&self, text: &str) -> anyhow::Result<EntityExtractionOutput> {
        let entities = self.entity_extractor.extract_entities(text).await?;
        Ok(EntityExtractionOutput { entities })
    }

    async fn relation_extraction(&self, text: &str) -> anyhow::Result<RelationExtractionOutput> {
        let entities = self.entity_extractor.extract_entities(text).await?;
        let relations = self
            .relation_extractor
            .extract_relations(text, &entities)
            .await?;
        Ok(RelationExtractionOutput {
            entities,
            relations,
        })
    }

    async fn ingestion(&self, text: &str, source: &str) -> anyhow::Result<IngestionOutput> {
        self.ensure_repo().await?;

        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();
        let resolver: &dyn EntityResolver = repo;

        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
            Some(resolver),
            None,
        )
        .await?;

        for inv in &result.diff.entities_invalidated {
            let existing = repo.find_entities_by_name(&inv.name, None, None).await?;
            for entity in existing {
                repo.invalidate_entity(entity.id).await?;
            }
        }
        for entity_id in &result.diff.entity_ids_to_invalidate_relations {
            repo.invalidate_relations_for_entity(*entity_id).await?;
        }

        repo.create_episode(&episode).await?;
        for entity in &result.entities {
            repo.upsert_entity(entity).await?;
        }
        for relation in &result.relations {
            repo.create_relation(relation).await?;
        }
        for memory in &result.memories {
            repo.create_memory(memory).await?;
        }

        Ok(IngestionOutput {
            entity_count: result.entities.len(),
            relation_count: result.relations.len(),
            memory_count: result.memories.len(),
            entities_created: result.diff.entities_created.len(),
            entities_updated: result.diff.entities_updated.len(),
        })
    }

    async fn search(&self, query: &str) -> anyhow::Result<SearchOutput> {
        self.ensure_repo().await?;

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();

        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = repo
            .search_entities_by_vector(&query_embedding, 10, None, None)
            .await?;
        let keyword_results = repo.search_entities_by_keyword(query, None, None).await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(SearchOutput {
            result_count: fused.len(),
        })
    }

    async fn query_rewrite(&self, query: &str) -> anyhow::Result<QueryRewriteOutput> {
        use context_keeper_core::traits::QueryRewriter;
        let variants = self.query_rewriter.rewrite(query).await?;
        Ok(QueryRewriteOutput { variants })
    }

    async fn search_entity_names(&self, query: &str) -> anyhow::Result<Vec<String>> {
        self.ensure_repo().await?;

        let guard = self.repo.lock().await;
        let repo = guard.as_ref().unwrap();

        let query_embedding = self.embedder.embed(query).await?;

        let vector_results = repo
            .search_entities_by_vector(&query_embedding, 10, None, None)
            .await?;
        let keyword_results = repo.search_entities_by_keyword(query, None, None).await?;

        let fused = fuse_rrf(vec![
            vector_results.into_iter().map(|(e, _)| e).collect(),
            keyword_results,
        ]);

        Ok(fused
            .into_iter()
            .filter_map(|sr| sr.entity.map(|e| e.name))
            .collect())
    }

    async fn reset(&self) -> anyhow::Result<()> {
        let mut guard = self.repo.lock().await;
        if guard.is_some() {
            let config = SurrealConfig {
                embedding_dimensions: self.embedding_dims,
                ..SurrealConfig::default()
            };
            let db = connect_memory(&config).await?;
            apply_schema(&db, &config).await?;
            *guard = Some(Repository::new(db));
        }
        Ok(())
    }
}
