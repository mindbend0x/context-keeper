use chrono::{DateTime, Utc};

use crate::models::{Entity, Relation};

/// Compute a staleness score for an entity based on how long ago
/// its associated facts were last confirmed.
pub fn staleness_score(entity: &Entity) -> f64 {
    let age = Utc::now() - entity.valid_from;
    age.num_days() as f64
}

/// A point-in-time snapshot of the knowledge graph.
#[derive(Debug)]
pub struct TemporalSnapshot {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    #[test]
    fn test_staleness_score() {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: "test".into(),
            entity_type: "test".into(),
            summary: "test".into(),
            embedding: vec![],
            valid_from: Utc::now() - Duration::days(10),
            valid_until: None,
        };
        let score = staleness_score(&entity);
        assert!((score - 10.0).abs() <= 1.0); // Allow 1-day tolerance
    }

    #[test]
    fn test_staleness_score_recent() {
        let entity = Entity {
            id: Uuid::new_v4(),
            name: "test".into(),
            entity_type: "test".into(),
            summary: "test".into(),
            embedding: vec![],
            valid_from: Utc::now(),
            valid_until: None,
        };
        assert_eq!(staleness_score(&entity), 0.0);
    }
}
