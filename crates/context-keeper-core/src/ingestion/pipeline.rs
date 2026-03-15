use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::models::{Entity, Episode, Memory, Relation};
use crate::traits::{Embedder, EntityExtractor, RelationExtractor};

/// The output of a successful ingestion run.
#[derive(Debug)]
pub struct IngestionResult {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub memories: Vec<Memory>,
}

/// Process an episode through the ingestion pipeline.
///
/// This is a pure-logic function that takes trait objects for LLM operations,
/// making it fully testable with mocks. The caller is responsible for
/// persisting the results to the repository.
///
/// Steps:
/// 1. Extract entities from the episode text
/// 2. Extract relations between entities
/// 3. Generate embeddings for each entity
/// 4. Create a memory (distilled fact) for the episode
pub async fn ingest(
    episode: &Episode,
    embedder: &dyn Embedder,
    entity_extractor: &dyn EntityExtractor,
    relation_extractor: &dyn RelationExtractor,
) -> Result<IngestionResult> {
    tracing::info!(episode_id = %episode.id, "Starting ingestion pipeline");

    // 1. Extract entities
    let extracted = entity_extractor
        .extract_entities(&episode.content)
        .await?;
    tracing::info!(count = extracted.len(), "Extracted entities");

    // 2. Build Entity models with embeddings
    let now = Utc::now();
    let mut entities = Vec::with_capacity(extracted.len());
    for ext in &extracted {
        let embedding = embedder.embed(&ext.name).await?;
        entities.push(Entity {
            id: Uuid::new_v4(),
            name: ext.name.clone(),
            entity_type: ext.entity_type.clone().into(),
            summary: ext.summary.clone(),
            embedding,
            valid_from: now,
            valid_until: None,
        });
    }

    // 3. Extract relations
    let extracted_rels = relation_extractor
        .extract_relations(&episode.content, &extracted)
        .await?;
    tracing::info!(count = extracted_rels.len(), "Extracted relations");

    // 4. Build Relation models, linking to entity IDs by name
    let mut relations = Vec::with_capacity(extracted_rels.len());
    for ext_rel in &extracted_rels {
        let source = entities.iter().find(|e| e.name == ext_rel.subject);
        let target = entities.iter().find(|e| e.name == ext_rel.object);
        if let (Some(src), Some(tgt)) = (source, target) {
            relations.push(Relation {
                id: Uuid::new_v4(),
                from_entity_id: src.id,
                to_entity_id: tgt.id,
                relation_type: ext_rel.predicate.clone(),
                confidence: ext_rel.confidence,
                valid_from: now,
                valid_until: None,
            });
        }
    }

    // 5. Create a memory for this episode
    let memory_embedding = embedder.embed(&episode.content).await?;
    let memory = Memory {
        id: Uuid::new_v4(),
        content: episode.content.clone(),
        embedding: memory_embedding,
        source_episode_id: episode.id,
        entity_ids: entities.iter().map(|e| e.id).collect(),
        created_at: now,
    };

    tracing::info!(
        entities = entities.len(),
        relations = relations.len(),
        "Ingestion complete"
    );

    Ok(IngestionResult {
        entities,
        relations,
        memories: vec![memory],
    })
}
