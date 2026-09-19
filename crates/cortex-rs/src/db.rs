use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

use crate::embeddings::{bytes_to_embedding, cosine_similarity, embedding_to_bytes};
use crate::models::*;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let _: Result<String, _> = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0));
        conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "BEGIN;
            CREATE TABLE IF NOT EXISTS spaces (
                id TEXT PRIMARY KEY,
                slug TEXT UNIQUE NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                space_id TEXT NOT NULL,
                space_slug TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                canonical_name TEXT NOT NULL,
                content TEXT NOT NULL,
                aliases_json TEXT NOT NULL DEFAULT '[]',
                metadata_json TEXT NOT NULL DEFAULT '{}',
                confidence REAL NOT NULL DEFAULT 1.0,
                revision INTEGER NOT NULL DEFAULT 1,
                retracted INTEGER NOT NULL DEFAULT 0,
                valid_from TEXT,
                valid_to TEXT,
                created_at TEXT NOT NULL,
                embedding BLOB
            );

            CREATE INDEX IF NOT EXISTS idx_entities_canonical ON entities(space_slug, canonical_name);
            CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(space_slug, entity_type);

            CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
                id UNINDEXED,
                canonical_name,
                content,
                space_slug UNINDEXED
            );

            CREATE TABLE IF NOT EXISTS claims (
                id TEXT PRIMARY KEY,
                space_id TEXT NOT NULL,
                space_slug TEXT NOT NULL,
                subject_entity_id TEXT NOT NULL,
                predicate TEXT NOT NULL,
                object_entity_id TEXT,
                literal_value_json TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                retracted INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_claims_subject ON claims(subject_entity_id);
            CREATE INDEX IF NOT EXISTS idx_claims_object ON claims(object_entity_id);
            COMMIT;",
        )?;

        // Ensure default space exists
        let space_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO spaces (id, slug, created_at) VALUES (?1, ?2, ?3)",
            params![space_id, "atlas-memory", now],
        );

        Ok(())
    }

    pub fn ensure_space(&self, slug: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM spaces WHERE slug = ?1")?;
        let mut rows = stmt.query(params![slug])?;
        if let Some(row) = rows.next()? {
            return row.get(0);
        }

        drop(rows);
        drop(stmt);

        let new_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO spaces (id, slug, created_at) VALUES (?1, ?2, ?3)",
            params![new_id, slug, now],
        )?;
        Ok(new_id)
    }

    pub fn upsert_entity(&self, input: &EntityWriteInput, embedding: Option<&[f32]>) -> Result<ReceiptDetails> {
        let space_id = self.ensure_space(&input.space)?;
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let embedding_blob = embedding.map(embedding_to_bytes);
        let aliases_json = serde_json::to_string(&input.aliases).unwrap_or_else(|_| "[]".to_string());
        let metadata_json = serde_json::to_string(&input.metadata).unwrap_or_else(|_| "{}".to_string());

        // Check if entity exists by canonical_name in space
        let mut stmt = conn.prepare("SELECT id, revision FROM entities WHERE space_slug = ?1 AND canonical_name = ?2")?;
        let mut rows = stmt.query(params![input.space, input.canonical_name])?;

        let (target_id, revision) = if let Some(row) = rows.next()? {
            let existing_id: String = row.get(0)?;
            let rev: i64 = row.get(1)?;
            (existing_id, rev + 1)
        } else {
            let new_id = input.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
            (new_id, 1)
        };
        drop(rows);
        drop(stmt);

        conn.execute(
            "INSERT INTO entities (id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                entity_type = excluded.entity_type,
                canonical_name = excluded.canonical_name,
                content = excluded.content,
                aliases_json = excluded.aliases_json,
                metadata_json = excluded.metadata_json,
                confidence = excluded.confidence,
                revision = excluded.revision,
                retracted = 0,
                valid_from = excluded.valid_from,
                valid_to = excluded.valid_to,
                embedding = COALESCE(excluded.embedding, entities.embedding)",
            params![
                target_id,
                space_id,
                input.space,
                input.entity_type,
                input.canonical_name,
                input.content,
                aliases_json,
                metadata_json,
                input.confidence,
                revision,
                input.valid_from,
                input.valid_to,
                now,
                embedding_blob,
            ],
        )?;

        // Update FTS table
        let _ = conn.execute("DELETE FROM entities_fts WHERE id = ?1", params![target_id]);
        let _ = conn.execute(
            "INSERT INTO entities_fts (id, canonical_name, content, space_slug) VALUES (?1, ?2, ?3, ?4)",
            params![target_id, input.canonical_name, input.content, input.space],
        );

        Ok(ReceiptDetails {
            id: Uuid::new_v4().to_string(),
            operation: "entity.upsert".to_string(),
            target_type: "entity".to_string(),
            target_id,
            committed_at: now,
        })
    }

    pub fn upsert_claim(&self, input: &ClaimWriteInput) -> Result<ReceiptDetails> {
        let space_id = self.ensure_space(&input.space)?;
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let claim_id = input.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let literal_json = input.literal_value.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());
        let metadata_json = serde_json::to_string(&input.metadata).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO claims (id, space_id, space_slug, subject_entity_id, predicate, object_entity_id, literal_value_json, confidence, metadata_json, retracted, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
             ON CONFLICT(id) DO UPDATE SET
                predicate = excluded.predicate,
                object_entity_id = excluded.object_entity_id,
                literal_value_json = excluded.literal_value_json,
                confidence = excluded.confidence,
                metadata_json = excluded.metadata_json,
                retracted = 0",
            params![
                claim_id,
                space_id,
                input.space,
                input.subject_entity_id,
                input.predicate,
                input.object_entity_id,
                literal_json,
                input.confidence,
                metadata_json,
                now,
            ],
        )?;

        Ok(ReceiptDetails {
            id: Uuid::new_v4().to_string(),
            operation: "claim.upsert".to_string(),
            target_type: "claim".to_string(),
            target_id: claim_id,
            committed_at: now,
        })
    }

    pub fn retract_target(&self, target_type: &str, target_id: &str) -> Result<ReceiptDetails> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        if target_type == "entity" {
            conn.execute("UPDATE entities SET retracted = 1 WHERE id = ?1", params![target_id])?;
            conn.execute("DELETE FROM entities_fts WHERE id = ?1", params![target_id])?;
        } else if target_type == "claim" {
            conn.execute("UPDATE claims SET retracted = 1 WHERE id = ?1", params![target_id])?;
        }

        Ok(ReceiptDetails {
            id: Uuid::new_v4().to_string(),
            operation: format!("{}.retract", target_type),
            target_type: target_type.to_string(),
            target_id: target_id.to_string(),
            committed_at: now,
        })
    }

    pub fn get_entity(&self, id_or_name: &str, space: Option<&str>) -> Result<Option<CortexEntity>> {
        let conn = self.conn.lock().unwrap();
        let target_space = space.unwrap_or("atlas-memory");

        let mut stmt = conn.prepare(
            "SELECT id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at
             FROM entities
             WHERE (id = ?1 OR canonical_name = ?1) AND space_slug = ?2 AND retracted = 0
             LIMIT 1"
        )?;

        let mut rows = stmt.query(params![id_or_name, target_space])?;
        if let Some(row) = rows.next()? {
            let aliases_str: String = row.get(6)?;
            let meta_str: String = row.get(7)?;
            Ok(Some(CortexEntity {
                id: row.get(0)?,
                space_id: row.get(1)?,
                space_slug: row.get(2)?,
                entity_type: row.get(3)?,
                canonical_name: row.get(4)?,
                content: row.get(5)?,
                aliases: serde_json::from_str(&aliases_str).unwrap_or_default(),
                metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                confidence: row.get(8)?,
                revision: row.get(9)?,
                retracted: row.get::<_, i64>(10)? != 0,
                valid_from: row.get(11)?,
                valid_to: row.get(12)?,
                created_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn search_entities(&self, query: &str, space: Option<&str>, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let target_space = space.unwrap_or("atlas-memory");
        let query_trimmed = query.trim();

        if query_trimmed.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at
                 FROM entities
                 WHERE space_slug = ?1 AND retracted = 0
                 ORDER BY created_at DESC
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![target_space, limit], |row| {
                let aliases_str: String = row.get(6)?;
                let meta_str: String = row.get(7)?;
                Ok(SearchResult {
                    entity: CortexEntity {
                        id: row.get(0)?,
                        space_id: row.get(1)?,
                        space_slug: row.get(2)?,
                        entity_type: row.get(3)?,
                        canonical_name: row.get(4)?,
                        content: row.get(5)?,
                        aliases: serde_json::from_str(&aliases_str).unwrap_or_default(),
                        metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                        confidence: row.get(8)?,
                        revision: row.get(9)?,
                        retracted: row.get::<_, i64>(10)? != 0,
                        valid_from: row.get(11)?,
                        valid_to: row.get(12)?,
                        created_at: row.get(13)?,
                    },
                    lexical_score: 0.4,
                    graph_score: 0.0,
                    semantic_score: 0.0,
                    score: 0.4,
                })
            })?;

            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            return Ok(results);
        }

        // FTS search with support for exact, prefix (*), and boolean operators (AND, OR, NOT)
        let clean_q = query_trimmed.replace('"', "");
        let has_fts_ops = clean_q.contains(" AND ")
            || clean_q.contains(" OR ")
            || clean_q.contains(" NOT ")
            || clean_q.contains('*');

        let mut results = Vec::new();

        let run_fts = |query_str: &str| -> Option<Vec<SearchResult>> {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.space_id, e.space_slug, e.entity_type, e.canonical_name, e.content, e.aliases_json, e.metadata_json, e.confidence, e.revision, e.retracted, e.valid_from, e.valid_to, e.created_at,
                        COALESCE(bm25(entities_fts), 5.0) as bm25_rank
                 FROM entities e
                 JOIN entities_fts f ON f.id = e.id
                 WHERE entities_fts MATCH ?1 AND e.space_slug = ?2 AND e.retracted = 0
                 ORDER BY bm25_rank ASC
                 LIMIT ?3"
            ).ok()?;

            let rows = stmt.query_map(params![query_str, target_space, limit], |row| {
                let aliases_str: String = row.get(6)?;
                let meta_str: String = row.get(7)?;
                let bm25_rank: f64 = row.get(14)?;
                let lexical_score = 1.0 / (1.0 + bm25_rank.abs());
                Ok(SearchResult {
                    entity: CortexEntity {
                        id: row.get(0)?,
                        space_id: row.get(1)?,
                        space_slug: row.get(2)?,
                        entity_type: row.get(3)?,
                        canonical_name: row.get(4)?,
                        content: row.get(5)?,
                        aliases: serde_json::from_str(&aliases_str).unwrap_or_default(),
                        metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                        confidence: row.get(8)?,
                        revision: row.get(9)?,
                        retracted: row.get::<_, i64>(10)? != 0,
                        valid_from: row.get(11)?,
                        valid_to: row.get(12)?,
                        created_at: row.get(13)?,
                    },
                    lexical_score,
                    graph_score: 0.0,
                    semantic_score: 0.0,
                    score: lexical_score,
                })
            }).ok()?;

            let mut out = Vec::new();
            for r in rows.flatten() {
                out.push(r);
            }
            Some(out)
        };

        if has_fts_ops {
            if let Some(r) = run_fts(&clean_q) {
                results = r;
            }
        }

        if results.is_empty() {
            let fts_query = format!("\"{}\"", clean_q);
            if let Some(r) = run_fts(&fts_query) {
                results = r;
            }
        }

        // Fallback to substring match if FTS did not return enough
        if results.len() < limit {
            let pattern = format!("%{}%", query_trimmed);
            let mut sub_stmt = conn.prepare(
                "SELECT id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at
                 FROM entities
                 WHERE (canonical_name LIKE ?1 OR content LIKE ?1) AND space_slug = ?2 AND retracted = 0
                 LIMIT ?3"
            )?;
            let sub_rows = sub_stmt.query_map(params![pattern, target_space, limit], |row| {
                let aliases_str: String = row.get(6)?;
                let meta_str: String = row.get(7)?;
                Ok(SearchResult {
                    entity: CortexEntity {
                        id: row.get(0)?,
                        space_id: row.get(1)?,
                        space_slug: row.get(2)?,
                        entity_type: row.get(3)?,
                        canonical_name: row.get(4)?,
                        content: row.get(5)?,
                        aliases: serde_json::from_str(&aliases_str).unwrap_or_default(),
                        metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                        confidence: row.get(8)?,
                        revision: row.get(9)?,
                        retracted: row.get::<_, i64>(10)? != 0,
                        valid_from: row.get(11)?,
                        valid_to: row.get(12)?,
                        created_at: row.get(13)?,
                    },
                    lexical_score: 0.7,
                    graph_score: 0.0,
                    semantic_score: 0.0,
                    score: 0.7,
                })
            })?;
            for r in sub_rows {
                let sr = r?;
                if !results.iter().any(|existing| existing.entity.id == sr.entity.id) {
                    results.push(sr);
                }
            }
        }

        results.truncate(limit);
        Ok(results)
    }

    pub fn recall_entities(&self, query_text: &str, query_embedding: Option<&[f32]>, space: Option<&str>, limit: usize) -> Result<Vec<RecallItem>> {
        let conn = self.conn.lock().unwrap();
        let target_space = space.unwrap_or("atlas-memory");

        let mut stmt = conn.prepare(
            "SELECT id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at, embedding
             FROM entities
             WHERE space_slug = ?1 AND retracted = 0"
        )?;

        let rows = stmt.query_map(params![target_space], |row| {
            let aliases_str: String = row.get(6)?;
            let meta_str: String = row.get(7)?;
            let embedding_blob: Option<Vec<u8>> = row.get(14)?;
            let entity = CortexEntity {
                id: row.get(0)?,
                space_id: row.get(1)?,
                space_slug: row.get(2)?,
                entity_type: row.get(3)?,
                canonical_name: row.get(4)?,
                content: row.get(5)?,
                aliases: serde_json::from_str(&aliases_str).unwrap_or_default(),
                metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                confidence: row.get(8)?,
                revision: row.get(9)?,
                retracted: row.get::<_, i64>(10)? != 0,
                valid_from: row.get(11)?,
                valid_to: row.get(12)?,
                created_at: row.get(13)?,
            };
            Ok((entity, embedding_blob))
        })?;

        let mut candidates = Vec::new();
        let query_lower = query_text.to_lowercase();

        for r in rows {
            let (entity, emb_blob) = r?;
            let mut semantic_score = 0.0f64;

            if let (Some(q_emb), Some(blob)) = (query_embedding, emb_blob) {
                let target_emb = bytes_to_embedding(&blob);
                semantic_score = cosine_similarity(q_emb, &target_emb) as f64;
            }

            // Calculate simple lexical score
            let name_lower = entity.canonical_name.to_lowercase();
            let content_lower = entity.content.to_lowercase();
            let mut lexical_score = 0.0f64;
            if name_lower.contains(&query_lower) {
                lexical_score = 0.9;
            } else if content_lower.contains(&query_lower) {
                lexical_score = 0.6;
            }

            let final_score = if query_embedding.is_some() {
                0.6 * semantic_score + 0.4 * lexical_score
            } else {
                lexical_score
            };

            candidates.push(RecallItem {
                entity,
                final_score,
                component_scores: RecallComponentScores {
                    lexical: lexical_score,
                    semantic: semantic_score,
                    confidence_recency: 0.95,
                    graph: 0.0,
                },
            });
        }

        candidates.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(limit);
        Ok(candidates)
    }

    pub fn traverse_claims(&self, subject_id: &str, space: Option<&str>) -> Result<Vec<CortexClaim>> {
        let conn = self.conn.lock().unwrap();
        let target_space = space.unwrap_or("atlas-memory");

        let mut stmt = conn.prepare(
            "SELECT id, space_id, space_slug, subject_entity_id, predicate, object_entity_id, literal_value_json, confidence, metadata_json, retracted, created_at
             FROM claims
             WHERE (subject_entity_id = ?1 OR object_entity_id = ?1) AND space_slug = ?2 AND retracted = 0"
        )?;

        let rows = stmt.query_map(params![subject_id, target_space], |row| {
            let lit_str: Option<String> = row.get(6)?;
            let meta_str: String = row.get(8)?;
            Ok(CortexClaim {
                id: row.get(0)?,
                space_id: row.get(1)?,
                space_slug: row.get(2)?,
                subject_entity_id: row.get(3)?,
                predicate: row.get(4)?,
                object_entity_id: row.get(5)?,
                literal_value: lit_str.and_then(|s| serde_json::from_str(&s).ok()),
                confidence: row.get(7)?,
                metadata: serde_json::from_str(&meta_str).unwrap_or_else(|_| serde_json::json!({})),
                retracted: row.get::<_, i64>(9)? != 0,
                created_at: row.get(10)?,
            })
        })?;

        let mut claims = Vec::new();
        for r in rows {
            claims.push(r?);
        }
        Ok(claims)
    }

    pub fn import_entity_raw(&self, entity: &CortexEntity, embedding: Option<&[f32]>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let embedding_blob = embedding.map(embedding_to_bytes);
        let aliases_json = serde_json::to_string(&entity.aliases).unwrap_or_else(|_| "[]".to_string());
        let metadata_json = serde_json::to_string(&entity.metadata).unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT INTO entities (id, space_id, space_slug, entity_type, canonical_name, content, aliases_json, metadata_json, confidence, revision, retracted, valid_from, valid_to, created_at, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                canonical_name = excluded.canonical_name,
                content = excluded.content,
                aliases_json = excluded.aliases_json,
                metadata_json = excluded.metadata_json,
                confidence = excluded.confidence,
                revision = excluded.revision,
                retracted = excluded.retracted,
                created_at = excluded.created_at,
                embedding = COALESCE(excluded.embedding, entities.embedding)",
            params![
                entity.id,
                entity.space_id,
                entity.space_slug,
                entity.entity_type,
                entity.canonical_name,
                entity.content,
                aliases_json,
                metadata_json,
                entity.confidence,
                entity.revision,
                if entity.retracted { 1 } else { 0 },
                entity.valid_from,
                entity.valid_to,
                entity.created_at,
                embedding_blob,
            ],
        )?;

        let _ = conn.execute("DELETE FROM entities_fts WHERE id = ?1", params![entity.id]);
        let _ = conn.execute(
            "INSERT INTO entities_fts (id, canonical_name, content, space_slug) VALUES (?1, ?2, ?3, ?4)",
            params![entity.id, entity.canonical_name, entity.content, entity.space_slug],
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_in_memory_db_crud() {
        let db = Database::open_in_memory().expect("open in memory");

        let input = EntityWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            entity_type: "discovery".to_string(),
            canonical_name: "test_cortex_rebuild".to_string(),
            content: "Testing the native Rust cortex-rs database engine".to_string(),
            aliases: vec!["cortex_test".to_string()],
            metadata: serde_json::json!({"test": true}),
            confidence: 1.0,
            valid_from: None,
            valid_to: None,
            external_id: None,
        };

        let dummy_emb = vec![0.5f32; 768];
        let receipt = db.upsert_entity(&input, Some(&dummy_emb)).expect("upsert entity");
        assert_eq!(receipt.operation, "entity.upsert");

        let fetched = db.get_entity("test_cortex_rebuild", Some("atlas-memory")).expect("get entity");
        assert!(fetched.is_some());
        let entity = fetched.unwrap();
        assert_eq!(entity.canonical_name, "test_cortex_rebuild");
        assert_eq!(entity.aliases.len(), 1);

        let search_results = db.search_entities("Rust", Some("atlas-memory"), 10).expect("search");
        assert!(!search_results.is_empty());
        assert_eq!(search_results[0].entity.canonical_name, "test_cortex_rebuild");

        let recall_results = db.recall_entities("Rust", Some(&dummy_emb), Some("atlas-memory"), 5).expect("recall");
        assert!(!recall_results.is_empty());
        assert!(recall_results[0].final_score > 0.5);
    }

    #[test]
    fn test_sqlite_wal_persistence_under_concurrency() {
        let temp_dir = std::env::temp_dir().join(format!("cortex_wal_test_{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("cortex.db");

        // Open database in WAL mode
        let db = Arc::new(Database::open(&db_path).expect("open wal db"));

        // Verify WAL mode is active
        {
            let conn = db.conn.lock().unwrap();
            let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
            assert_eq!(mode.to_lowercase(), "wal");
        }

        let mut handles = Vec::new();

        // Spawn 4 concurrent writer threads
        for t in 0..4 {
            let db_clone = Arc::clone(&db);
            handles.push(thread::spawn(move || {
                for i in 0..10 {
                    let input = EntityWriteInput {
                        id: None,
                        space: "atlas-memory".to_string(),
                        entity_type: "concurrent_write".to_string(),
                        canonical_name: format!("entity_t{}_i{}", t, i),
                        content: format!("Content from thread {} iteration {}", t, i),
                        aliases: vec![format!("alias_t{}_i{}", t, i)],
                        metadata: serde_json::json!({"thread": t, "iter": i}),
                        confidence: 0.95,
                        valid_from: None,
                        valid_to: None,
                        external_id: None,
                    };
                    let receipt = db_clone.upsert_entity(&input, None).expect("concurrent entity upsert");
                    assert_eq!(receipt.operation, "entity.upsert");

                    // Also write a claim
                    let claim_input = ClaimWriteInput {
                        id: None,
                        space: "atlas-memory".to_string(),
                        subject_entity_id: receipt.target_id.clone(),
                        predicate: "authored_by".to_string(),
                        object_entity_id: None,
                        literal_value: Some(serde_json::json!(format!("worker_thread_{}", t))),
                        confidence: 1.0,
                        metadata: serde_json::json!({}),
                    };
                    let claim_receipt = db_clone.upsert_claim(&claim_input).expect("concurrent claim upsert");
                    assert_eq!(claim_receipt.operation, "claim.upsert");
                }
            }));
        }

        // Spawn 4 concurrent reader threads
        for _ in 0..4 {
            let db_clone = Arc::clone(&db);
            handles.push(thread::spawn(move || {
                for _ in 0..15 {
                    let _ = db_clone.search_entities("Content", Some("atlas-memory"), 10);
                    let _ = db_clone.recall_entities("thread", None, Some("atlas-memory"), 5);
                    let _ = db_clone.get_entity("entity_t0_i0", Some("atlas-memory"));
                    thread::sleep(std::time::Duration::from_millis(2));
                }
            }));
        }

        // Wait for all threads to join
        for h in handles {
            h.join().expect("thread join");
        }

        // Close db by dropping Arc
        drop(db);

        // Re-open from disk to verify cold-start WAL recovery and persistence
        let reopened = Database::open(&db_path).expect("reopen wal db");
        for t in 0..4 {
            for i in 0..10 {
                let name = format!("entity_t{}_i{}", t, i);
                let ent = reopened.get_entity(&name, Some("atlas-memory")).expect("get persisted entity");
                assert!(ent.is_some(), "Entity {} must persist across restarts", name);
                let e = ent.unwrap();
                assert_eq!(e.canonical_name, name);
                assert_eq!(e.content, format!("Content from thread {} iteration {}", t, i));
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_fts5_exact_prefix_boolean_and_ranking() {
        let db = Database::open_in_memory().expect("open in memory db");

        let items = vec![
            ("NVFP4_Loader", "High throughput native NVFP4 checkpoint loader for vLLM and MAX", "discovery"),
            ("Qwen3_Parser", "Llama cpp tokenizer and grammar parser for Qwen3 models", "learned_procedure"),
            ("TPM_Vault", "Hardware TPM bound key vault with zero plaintext secrets on disk", "lesson"),
            ("Sovereign_Covenant", "Frontier AI Anti Enclosure mandate and sovereign defense network", "discovery"),
            ("Memory_Optimizer", "Memory architecture optimization for memory engines with memory reuse", "lesson"),
        ];

        for (name, content, etype) in items {
            let input = EntityWriteInput {
                id: None,
                space: "atlas-memory".to_string(),
                entity_type: etype.to_string(),
                canonical_name: name.to_string(),
                content: content.to_string(),
                aliases: vec![],
                metadata: serde_json::json!({}),
                confidence: 1.0,
                valid_from: None,
                valid_to: None,
                external_id: None,
            };
            db.upsert_entity(&input, None).expect("insert entity");
        }

        // 1. Exact term matching
        let exact_res = db.search_entities("NVFP4", Some("atlas-memory"), 10).expect("search exact");
        assert_eq!(exact_res.len(), 1);
        assert_eq!(exact_res[0].entity.canonical_name, "NVFP4_Loader");

        let exact_tpm = db.search_entities("TPM", Some("atlas-memory"), 10).expect("search exact TPM");
        assert_eq!(exact_tpm.len(), 1);
        assert_eq!(exact_tpm[0].entity.canonical_name, "TPM_Vault");

        // 2. Prefix queries
        let prefix_res = db.search_entities("NVF*", Some("atlas-memory"), 10).expect("search prefix");
        assert_eq!(prefix_res.len(), 1);
        assert_eq!(prefix_res[0].entity.canonical_name, "NVFP4_Loader");

        let prefix_hard = db.search_entities("hardw*", Some("atlas-memory"), 10).expect("search prefix hardw*");
        assert_eq!(prefix_hard.len(), 1);
        assert_eq!(prefix_hard[0].entity.canonical_name, "TPM_Vault");

        let prefix_token = db.search_entities("token*", Some("atlas-memory"), 10).expect("search prefix token*");
        assert_eq!(prefix_token.len(), 1);
        assert_eq!(prefix_token[0].entity.canonical_name, "Qwen3_Parser");

        // 3. Boolean AND
        let and_res = db.search_entities("NVFP4 AND vLLM", Some("atlas-memory"), 10).expect("search AND");
        assert_eq!(and_res.len(), 1);
        assert_eq!(and_res[0].entity.canonical_name, "NVFP4_Loader");

        let and_mismatch = db.search_entities("NVFP4 AND non_existent_token", Some("atlas-memory"), 10).expect("search AND mismatch");
        assert_eq!(and_mismatch.len(), 0);

        // 4. Boolean OR
        let or_res = db.search_entities("Qwen3 OR TPM", Some("atlas-memory"), 10).expect("search OR");
        assert_eq!(or_res.len(), 2);
        let names: Vec<String> = or_res.into_iter().map(|r| r.entity.canonical_name).collect();
        assert!(names.contains(&"Qwen3_Parser".to_string()));
        assert!(names.contains(&"TPM_Vault".to_string()));

        // 5. Boolean NOT
        let not_res = db.search_entities("native NOT tokenizer", Some("atlas-memory"), 10).expect("search NOT");
        assert_eq!(not_res.len(), 1);
        assert_eq!(not_res[0].entity.canonical_name, "NVFP4_Loader");

        // 6. Ranking test (BM25 term frequency)
        let rank_res = db.search_entities("memory", Some("atlas-memory"), 10).expect("search ranking");
        assert!(!rank_res.is_empty());
        assert_eq!(rank_res[0].entity.canonical_name, "Memory_Optimizer");
    }

    #[test]
    fn test_vector_similarity_and_nearest_neighbor_ranking() {
        let db = Database::open_in_memory().expect("open in memory db");

        // Create 3 vectors with 768 dimensions
        let mut emb_a = vec![0.0f32; 768];
        emb_a[0] = 1.0; // Direction X

        let mut emb_b = vec![0.0f32; 768];
        emb_b[0] = 0.7071; // Direction between X and Y
        emb_b[1] = 0.7071;

        let mut emb_c = vec![0.0f32; 768];
        emb_c[1] = 1.0; // Direction Y

        let entities = vec![
            ("Vector_Alpha", "Entity aligned with primary axis X", emb_a),
            ("Vector_Beta", "Entity aligned midway between axes", emb_b),
            ("Vector_Gamma", "Entity aligned with secondary axis Y", emb_c),
        ];

        for (name, content, emb) in entities {
            let input = EntityWriteInput {
                id: None,
                space: "atlas-memory".to_string(),
                entity_type: "vector_item".to_string(),
                canonical_name: name.to_string(),
                content: content.to_string(),
                aliases: vec![],
                metadata: serde_json::json!({}),
                confidence: 1.0,
                valid_from: None,
                valid_to: None,
                external_id: None,
            };
            db.upsert_entity(&input, Some(&emb)).expect("insert vector entity");
        }

        // Query vector strongly aligned with X: [0.99, 0.05, 0, ...]
        let mut query_emb = vec![0.0f32; 768];
        query_emb[0] = 0.99;
        query_emb[1] = 0.05;

        let recall_res = db.recall_entities("Entity", Some(&query_emb), Some("atlas-memory"), 3).expect("recall");
        assert_eq!(recall_res.len(), 3);

        // Verify nearest neighbor ordering: Alpha first, Beta second, Gamma third
        assert_eq!(recall_res[0].entity.canonical_name, "Vector_Alpha");
        assert_eq!(recall_res[1].entity.canonical_name, "Vector_Beta");
        assert_eq!(recall_res[2].entity.canonical_name, "Vector_Gamma");

        assert!(recall_res[0].final_score > recall_res[1].final_score);
        assert!(recall_res[1].final_score > recall_res[2].final_score);

        // Empty embedding fallback to lexical only
        let empty_recall = db.recall_entities("secondary", None, Some("atlas-memory"), 3).expect("empty emb recall");
        assert!(!empty_recall.is_empty());
        assert_eq!(empty_recall[0].entity.canonical_name, "Vector_Gamma");
    }

    #[test]
    fn test_schema_integrity_metadata_indexing_and_deduplication() {
        let db = Database::open_in_memory().expect("open in memory db");

        // 1. Entity deduplication test
        let input_v1 = EntityWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            entity_type: "discovery".to_string(),
            canonical_name: "Atlas_Vault_Spec".to_string(),
            content: "Initial specification v1".to_string(),
            aliases: vec!["vault_v1".to_string()],
            metadata: serde_json::json!({
                "subsystem": "security",
                "hardware": {"tpm": true, "vendor": "stmicroelectronics"}
            }),
            confidence: 0.9,
            valid_from: None,
            valid_to: None,
            external_id: None,
        };

        let receipt1 = db.upsert_entity(&input_v1, None).expect("first upsert");
        let initial_id = receipt1.target_id.clone();

        // Re-upsert with same canonical_name in same space
        let input_v2 = EntityWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            entity_type: "learned_procedure".to_string(),
            canonical_name: "Atlas_Vault_Spec".to_string(),
            content: "Updated specification v2 with hardware sealing".to_string(),
            aliases: vec!["vault_v1".to_string(), "vault_v2".to_string()],
            metadata: serde_json::json!({
                "subsystem": "security",
                "hardware": {"tpm": true, "vendor": "stmicroelectronics", "pcr_sealing": [0, 2, 7]}
            }),
            confidence: 1.0,
            valid_from: None,
            valid_to: None,
            external_id: None,
        };

        let receipt2 = db.upsert_entity(&input_v2, None).expect("second upsert");

        // Verify deduplication: ID remains unchanged, revision incremented to 2
        assert_eq!(receipt2.target_id, initial_id);

        let fetched = db.get_entity("Atlas_Vault_Spec", Some("atlas-memory")).expect("get entity").unwrap();
        assert_eq!(fetched.id, initial_id);
        assert_eq!(fetched.revision, 2);
        assert_eq!(fetched.entity_type, "learned_procedure");
        assert_eq!(fetched.content, "Updated specification v2 with hardware sealing");
        assert_eq!(fetched.aliases.len(), 2);

        // Verify nested metadata integrity
        assert_eq!(fetched.metadata["subsystem"], "security");
        assert_eq!(fetched.metadata["hardware"]["tpm"], true);
        assert_eq!(fetched.metadata["hardware"]["pcr_sealing"].as_array().unwrap().len(), 3);

        // Verify exactly 1 entity row in space
        {
            let conn = db.conn.lock().unwrap();
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM entities WHERE space_slug = 'atlas-memory' AND canonical_name = 'Atlas_Vault_Spec'",
                [],
                |r| r.get(0),
            ).unwrap();
            assert_eq!(count, 1);
        }

        // 2. Claim schema and foreign key traversal
        let obj_input = EntityWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            entity_type: "component".to_string(),
            canonical_name: "TPM_Chip".to_string(),
            content: "Discrete TPM 2.0 module".to_string(),
            aliases: vec![],
            metadata: serde_json::json!({}),
            confidence: 1.0,
            valid_from: None,
            valid_to: None,
            external_id: None,
        };
        let obj_receipt = db.upsert_entity(&obj_input, None).expect("upsert object entity");

        let claim_input = ClaimWriteInput {
            id: None,
            space: "atlas-memory".to_string(),
            subject_entity_id: initial_id.clone(),
            predicate: "binds_to".to_string(),
            object_entity_id: Some(obj_receipt.target_id.clone()),
            literal_value: None,
            confidence: 1.0,
            metadata: serde_json::json!({"interface": "SPI"}),
        };
        db.upsert_claim(&claim_input).expect("upsert claim");

        let claims = db.traverse_claims(&initial_id, Some("atlas-memory")).expect("traverse claims");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].predicate, "binds_to");
        assert_eq!(claims[0].object_entity_id, Some(obj_receipt.target_id));
        assert_eq!(claims[0].metadata["interface"], "SPI");

        // 3. Retraction verification
        let retract_receipt = db.retract_target("entity", &initial_id).expect("retract");
        assert_eq!(retract_receipt.operation, "entity.retract");

        let after_retract = db.get_entity("Atlas_Vault_Spec", Some("atlas-memory")).expect("get retracted");
        assert!(after_retract.is_none());

        let search_retract = db.search_entities("Atlas_Vault_Spec", Some("atlas-memory"), 10).expect("search retracted");
        assert!(search_retract.is_empty());
    }
}
