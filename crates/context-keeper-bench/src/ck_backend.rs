use async_trait::async_trait;
use chrono::Utc;
use context_keeper_core::ingestion;
use context_keeper_core::models::Episode;
use context_keeper_core::traits::{EntityExtractor, RelationExtractor};
use context_keeper_rig::embeddings::RigEmbedder;
use context_keeper_rig::extraction::{RigEntityExtractor, RigRelationExtractor};
use context_keeper_rig::rewriting::RigQueryRewriter;
use uuid::Uuid;

use crate::backend::{
    BenchBackend, EntityExtractionOutput, IngestionOutput, QueryRewriteOutput,
    RelationExtractionOutput, SearchOutput,
};
use crate::config::ProviderConfig;

/// Context Keeper backend that delegates to real Rig-based extractors.
pub struct ContextKeeperBackend {
    provider_name: String,
    entity_extractor: RigEntityExtractor,
    relation_extractor: RigRelationExtractor,
    embedder: RigEmbedder,
    query_rewriter: RigQueryRewriter,
}

impl ContextKeeperBackend {
    pub fn from_provider(provider: &ProviderConfig) -> Self {
        let entity_extractor =
            RigEntityExtractor::new(&provider.api_url, &provider.api_key, &provider.extraction_model);
        let relation_extractor =
            RigRelationExtractor::new(&provider.api_url, &provider.api_key, &provider.extraction_model);
        let embedder = RigEmbedder::new(
            &provider.api_url,
            &provider.api_key,
            &provider.embedding_model,
            provider.embedding_dims,
        );
        let query_rewriter =
            RigQueryRewriter::new(&provider.api_url, &provider.api_key, &provider.extraction_model);

        Self {
            provider_name: provider.name.clone(),
            entity_extractor,
            relation_extractor,
            embedder,
            query_rewriter,
        }
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
        let episode = Episode {
            id: Uuid::new_v4(),
            content: text.to_string(),
            source: source.to_string(),
            session_id: None,
            agent: None,
            namespace: None,
            created_at: Utc::now(),
        };

        let result = ingestion::ingest(
            &episode,
            &self.embedder,
            &self.entity_extractor,
            &self.relation_extractor,
            None,
            None,
        )
        .await?;

        Ok(IngestionOutput {
            entity_count: result.entities.len(),
            relation_count: result.relations.len(),
            memory_count: result.memories.len(),
            entities_created: result.diff.entities_created.len(),
            entities_updated: result.diff.entities_updated.len(),
        })
    }

    async fn search(&self, query: &str) -> anyhow::Result<SearchOutput> {
        use context_keeper_core::traits::QueryRewriter;
        let variants = self.query_rewriter.rewrite(query).await?;
        Ok(SearchOutput {
            result_count: variants.len(),
        })
    }

    async fn query_rewrite(&self, query: &str) -> anyhow::Result<QueryRewriteOutput> {
        use context_keeper_core::traits::QueryRewriter;
        let variants = self.query_rewriter.rewrite(query).await?;
        Ok(QueryRewriteOutput { variants })
    }
}
