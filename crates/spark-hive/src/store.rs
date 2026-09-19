use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

use crate::geometry::{hex_distance, HexCoord, HEX_DIRECTIONS, HEX_DIRECTION_LABELS};

#[derive(Debug, Error)]
pub enum HiveError {
    #[error("Coordinate ({q}, {r}) is already occupied by comb '{comb_id}'")]
    Collision { q: i32, r: i32, comb_id: String },
    #[error("Parent comb '{0}' not found")]
    ParentNotFound(String),
    #[error("SQLite database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Forge task '{0}' not found")]
    TaskNotFound(String),
    #[error("Forge task '{task_id}' is already claimed by '{claimed_by}'")]
    TaskAlreadyClaimed { task_id: String, claimed_by: String },
    #[error("Unauthorized: task '{task_id}' is leased to '{claimed_by}', not '{agent_id}'")]
    TaskUnauthorized { task_id: String, claimed_by: String, agent_id: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateForgeTaskInput {
    pub project: String,
    pub module: String,
    pub ring: usize,
    pub title: String,
    pub description: String,
    pub parent_comb_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeTask {
    pub id: String,
    pub comb_id: String,
    pub project: String,
    pub module: String,
    pub ring: usize,
    pub q: i32,
    pub r: i32,
    pub title: String,
    pub status: String,
    pub claimed_by: Option<String>,
    pub expires_at: Option<String>,
    pub worktree_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveComb {
    pub id: String,
    pub q: i32,
    pub r: i32,
    pub author: String,
    pub role: String,
    pub content: String,
    pub intent: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbors: Vec<NeighborLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborLink {
    pub id: String,
    pub q: i32,
    pub r: i32,
    pub direction: usize,
    pub direction_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_q: i32,
    pub max_q: i32,
    pub min_r: i32,
    pub max_r: i32,
    pub count: usize,
    pub radius: i32,
    pub revision: usize,
    pub message_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaceCombInput {
    pub q: Option<i32>,
    pub r: Option<i32>,
    pub author: String,
    pub role: Option<String>,
    pub content: String,
    pub intent: Option<String>,
    pub parent_id: Option<String>,
}

pub struct CombStore {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl CombStore {
    pub fn open(path: &Path) -> Result<Self, HiveError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        let _: Result<String, _> = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0));
        conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let store = Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_default() -> Result<Self, HiveError> {
        let path_str = std::env::var("HIVE_DB_PATH")
            .unwrap_or_else(|_| "/home/drakestapleton/.config/cortex/hive.db".to_string());
        let path = PathBuf::from(path_str);
        Self::open(&path)
    }

    pub fn open_in_memory() -> Result<Self, HiveError> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn init_schema(&self) -> Result<(), HiveError> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hive_combs (
                id TEXT PRIMARY KEY,
                q INTEGER NOT NULL,
                r INTEGER NOT NULL,
                author TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'agent',
                content TEXT NOT NULL,
                intent TEXT NOT NULL DEFAULT 'independent',
                parent_id TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(q, r)
            );
            CREATE INDEX IF NOT EXISTS idx_hive_combs_qr ON hive_combs(q, r);
            CREATE INDEX IF NOT EXISTS idx_hive_combs_created ON hive_combs(created_at);
            CREATE INDEX IF NOT EXISTS idx_hive_combs_parent ON hive_combs(parent_id);
            CREATE TABLE IF NOT EXISTS forge_tasks (
                id TEXT PRIMARY KEY,
                comb_id TEXT NOT NULL,
                project TEXT NOT NULL,
                module TEXT NOT NULL,
                ring INTEGER NOT NULL,
                q INTEGER NOT NULL,
                r INTEGER NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                claimed_by TEXT,
                expires_at TEXT,
                worktree_path TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_forge_tasks_status ON forge_tasks(status);
            CREATE INDEX IF NOT EXISTS idx_forge_tasks_project ON forge_tasks(project);"
        )?;

        // Seed genesis comb if table is empty
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM hive_combs", [], |r| r.get(0))?;
        if count == 0 {
            let genesis_id = "comb-genesis-00000000";
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO hive_combs (id, q, r, author, role, content, intent, parent_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    genesis_id,
                    0,
                    0,
                    "AIEN Genesis",
                    "coordinator",
                    "AIEN Sovereign Hive Online · Hexagonal Honeycomb Active",
                    "independent",
                    None::<String>,
                    now,
                ],
            )?;
        }

        Ok(())
    }

    pub fn get_comb(&self, id: &str) -> Result<Option<HiveComb>, HiveError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, q, r, author, role, content, intent, parent_id, created_at
             FROM hive_combs WHERE id = ?1"
        )?;
        let row = stmt.query_row(params![id], |r| {
            Ok(HiveComb {
                id: r.get(0)?,
                q: r.get(1)?,
                r: r.get(2)?,
                author: r.get(3)?,
                role: r.get(4)?,
                content: r.get(5)?,
                intent: r.get(6)?,
                parent_id: r.get(7)?,
                created_at: r.get(8)?,
                neighbors: Vec::new(),
            })
        }).optional()?;

        Ok(row)
    }

    pub fn get_comb_at(&self, q: i32, r: i32) -> Result<Option<HiveComb>, HiveError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, q, r, author, role, content, intent, parent_id, created_at
             FROM hive_combs WHERE q = ?1 AND r = ?2"
        )?;
        let row = stmt.query_row(params![q, r], |row| {
            Ok(HiveComb {
                id: row.get(0)?,
                q: row.get(1)?,
                r: row.get(2)?,
                author: row.get(3)?,
                role: row.get(4)?,
                content: row.get(5)?,
                intent: row.get(6)?,
                parent_id: row.get(7)?,
                created_at: row.get(8)?,
                neighbors: Vec::new(),
            })
        }).optional()?;

        Ok(row)
    }

    pub fn get_cells(&self) -> Result<(Vec<HiveComb>, BoundingBox), HiveError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, q, r, author, role, content, intent, parent_id, created_at
             FROM hive_combs ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map([], |r| {
            Ok(HiveComb {
                id: r.get(0)?,
                q: r.get(1)?,
                r: r.get(2)?,
                author: r.get(3)?,
                role: r.get(4)?,
                content: r.get(5)?,
                intent: r.get(6)?,
                parent_id: r.get(7)?,
                created_at: r.get(8)?,
                neighbors: Vec::new(),
            })
        })?;

        let mut combs = Vec::new();
        let mut min_q = 0;
        let mut max_q = 0;
        let mut min_r = 0;
        let mut max_r = 0;
        let mut max_radius = 5;

        for (i, comb_res) in rows.enumerate() {
            let comb = comb_res?;
            if i == 0 {
                min_q = comb.q;
                max_q = comb.q;
                min_r = comb.r;
                max_r = comb.r;
            } else {
                min_q = min_q.min(comb.q);
                max_q = max_q.max(comb.q);
                min_r = min_r.min(comb.r);
                max_r = max_r.max(comb.r);
            }
            let d = hex_distance(comb.q, comb.r, 0, 0);
            if d > max_radius {
                max_radius = d;
            }
            combs.push(comb);
        }

        // Build coordinate lookup table for neighbor linking
        let coord_map: std::collections::HashMap<(i32, i32), (String, usize)> = combs
            .iter()
            .enumerate()
            .map(|(idx, c)| ((c.q, c.r), (c.id.clone(), idx)))
            .collect();

        // Calculate neighbor links for each comb
        for comb in &mut combs {
            let mut neighbors = Vec::new();
            for (dir_idx, (dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
                let n_coord = (comb.q + dq, comb.r + dr);
                if let Some((n_id, _)) = coord_map.get(&n_coord) {
                    neighbors.push(NeighborLink {
                        id: n_id.clone(),
                        q: n_coord.0,
                        r: n_coord.1,
                        direction: dir_idx,
                        direction_label: HEX_DIRECTION_LABELS[dir_idx].to_string(),
                    });
                }
            }
            comb.neighbors = neighbors;
        }

        let count = combs.len();
        let bounds = BoundingBox {
            min_q,
            max_q,
            min_r,
            max_r,
            count,
            radius: max_radius,
            revision: count,
            message_count: count,
        };

        Ok((combs, bounds))
    }

    /// Finds the first available unoccupied coordinate.
    /// If parent_id is given, searches adjacent neighbors of parent first.
    pub fn find_free_coordinate(&self, parent_id: Option<&str>) -> Result<HexCoord, HiveError> {
        let conn = self.conn.lock().unwrap();
        Self::find_free_coordinate_with_conn(&conn, parent_id)
    }

    /// Internal coordinate finder operating under an already locked database connection.
    fn find_free_coordinate_with_conn(
        conn: &rusqlite::Connection,
        parent_id: Option<&str>,
    ) -> Result<HexCoord, HiveError> {
        let mut stmt = conn.prepare("SELECT q, r FROM hive_combs")?;
        let occupied_coords: HashSet<(i32, i32)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<HashSet<_>, _>>()?;

        let center = if let Some(pid) = parent_id {
            let mut parent_stmt = conn.prepare("SELECT q, r FROM hive_combs WHERE id = ?1")?;
            let parent_coord: Option<(i32, i32)> = parent_stmt
                .query_row(params![pid], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;

            match parent_coord {
                Some((pq, pr)) => HexCoord::new(pq, pr),
                None => HexCoord::new(0, 0),
            }
        } else {
            HexCoord::new(0, 0)
        };

        // Check center itself first if empty
        if !occupied_coords.contains(&(center.q, center.r)) {
            return Ok(center);
        }

        // 1. Check all 6 immediate neighbors in canonical order
        for (dq, dr) in HEX_DIRECTIONS {
            let cand = (center.q + dq, center.r + dr);
            if !occupied_coords.contains(&cand) {
                return Ok(HexCoord::new(cand.0, cand.1));
            }
        }

        // 2. Search outward rings around center up to radius 50
        for ring_radius in 2..=50 {
            let mut current = HexCoord::new(
                center.q + HEX_DIRECTIONS[4].0 * ring_radius,
                center.r + HEX_DIRECTIONS[4].1 * ring_radius,
            );
            for (dir_idx, _) in HEX_DIRECTIONS.iter().enumerate() {
                for _ in 0..ring_radius {
                    if !occupied_coords.contains(&(current.q, current.r)) {
                        return Ok(current);
                    }
                    let next_dir = HEX_DIRECTIONS[dir_idx];
                    current = HexCoord::new(current.q + next_dir.0, current.r + next_dir.1);
                }
            }
        }

        Ok(HexCoord::new(0, 0))
    }

    pub fn place_comb(&self, input: PlaceCombInput) -> Result<HiveComb, HiveError> {
        let conn = self.conn.lock().unwrap();

        let (q, r) = match (input.q, input.r) {
            (Some(q), Some(r)) => (q, r),
            _ => {
                let coord = Self::find_free_coordinate_with_conn(&conn, input.parent_id.as_deref())?;
                (coord.q, coord.r)
            }
        };

        // 1. Collision check
        let mut check_stmt = conn.prepare("SELECT id FROM hive_combs WHERE q = ?1 AND r = ?2")?;
        let existing_id: Option<String> = check_stmt.query_row(params![q, r], |row| row.get(0)).optional()?;
        if let Some(id) = existing_id {
            return Err(HiveError::Collision { q, r, comb_id: id });
        }

        // 2. Neighbor linking
        let mut neighbors = Vec::new();
        let mut neighbor_stmt = conn.prepare("SELECT id FROM hive_combs WHERE q = ?1 AND r = ?2")?;
        for (dir_idx, (dq, dr)) in HEX_DIRECTIONS.iter().enumerate() {
            let n_q = q + dq;
            let n_r = r + dr;
            if let Some(n_id) = neighbor_stmt.query_row(params![n_q, n_r], |row| row.get(0)).optional()? {
                neighbors.push(NeighborLink {
                    id: n_id,
                    q: n_q,
                    r: n_r,
                    direction: dir_idx,
                    direction_label: HEX_DIRECTION_LABELS[dir_idx].to_string(),
                });
            }
        }

        // 3. Verify parent_id if supplied
        if let Some(ref pid) = input.parent_id {
            let mut parent_check = conn.prepare("SELECT 1 FROM hive_combs WHERE id = ?1")?;
            let parent_exists: Option<i32> = parent_check.query_row(params![pid], |_| Ok(1)).optional()?;
            if parent_exists.is_none() {
                // Parent does not exist yet; accepted as unanchored
            }
        }

        let id = format!("comb-{}", Uuid::new_v4());
        let role = input.role.unwrap_or_else(|| "agent".to_string());
        let intent = input.intent.unwrap_or_else(|| "independent".to_string());
        let created_at = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO hive_combs (id, q, r, author, role, content, intent, parent_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &id,
                q,
                r,
                &input.author,
                &role,
                &input.content,
                &intent,
                &input.parent_id,
                &created_at,
            ],
        )?;

        Ok(HiveComb {
            id,
            q,
            r,
            author: input.author,
            role,
            content: input.content,
            intent,
            parent_id: input.parent_id,
            created_at,
            neighbors,
        })
    }

    pub fn spawn_forge_project(
        &self,
        project: &str,
        title: &str,
        core_spec: &str,
    ) -> Result<ForgeTask, HiveError> {
        let comb = self.place_comb(PlaceCombInput {
            q: None,
            r: None,
            author: "AIEN-Forge".to_string(),
            role: Some("genesis".to_string()),
            content: format!("Project [{}] core spec: {}", project, core_spec),
            intent: Some("genesis".to_string()),
            parent_id: None,
        })?;

        let task_id = format!("task-{}-{}", project, &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO forge_tasks (id, comb_id, project, module, ring, q, r, title, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, 'open', ?8)",
            params![
                task_id,
                comb.id,
                project,
                "core",
                comb.q,
                comb.r,
                title,
                now,
            ],
        )?;

        Ok(ForgeTask {
            id: task_id,
            comb_id: comb.id,
            project: project.to_string(),
            module: "core".to_string(),
            ring: 0,
            q: comb.q,
            r: comb.r,
            title: title.to_string(),
            status: "open".to_string(),
            claimed_by: None,
            expires_at: None,
            worktree_path: None,
            created_at: now,
        })
    }

    pub fn create_forge_task(
        &self,
        input: CreateForgeTaskInput,
    ) -> Result<ForgeTask, HiveError> {
        let comb = self.place_comb(PlaceCombInput {
            q: None,
            r: None,
            author: "AIEN-Forge".to_string(),
            role: Some("module".to_string()),
            content: format!("Task [{}:{}]: {}", input.project, input.module, input.description),
            intent: Some("branch".to_string()),
            parent_id: input.parent_comb_id.clone().or_else(|| Some("comb-genesis-00000000".to_string())),
        })?;

        let task_id = format!("task-{}-{}", input.module, &Uuid::new_v4().to_string()[..8]);
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO forge_tasks (id, comb_id, project, module, ring, q, r, title, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', ?9)",
            params![
                task_id,
                comb.id,
                input.project,
                input.module,
                input.ring as i64,
                comb.q,
                comb.r,
                input.title,
                now,
            ],
        )?;

        Ok(ForgeTask {
            id: task_id,
            comb_id: comb.id,
            project: input.project,
            module: input.module,
            ring: input.ring,
            q: comb.q,
            r: comb.r,
            title: input.title,
            status: "open".to_string(),
            claimed_by: None,
            expires_at: None,
            worktree_path: None,
            created_at: now,
        })
    }

    pub fn claim_forge_task(
        &self,
        task_id: &str,
        agent_id: &str,
        ttl_secs: u64,
    ) -> Result<ForgeTask, HiveError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, comb_id, project, module, ring, q, r, title, status, claimed_by, expires_at, worktree_path, created_at
             FROM forge_tasks WHERE id = ?1"
        )?;

        let mut row_iter = stmt.query_map(params![task_id], |row| {
            Ok(ForgeTask {
                id: row.get(0)?,
                comb_id: row.get(1)?,
                project: row.get(2)?,
                module: row.get(3)?,
                ring: row.get::<_, i64>(4)? as usize,
                q: row.get(5)?,
                r: row.get(6)?,
                title: row.get(7)?,
                status: row.get(8)?,
                claimed_by: row.get(9)?,
                expires_at: row.get(10)?,
                worktree_path: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;

        let mut task = match row_iter.next() {
            Some(res) => res?,
            None => return Err(HiveError::TaskNotFound(task_id.to_string())),
        };

        let now = Utc::now();
        let now_str = now.to_rfc3339();

        if task.status == "claimed" {
            if let Some(ref exp) = task.expires_at {
                if exp.as_str() > now_str.as_str() {
                    if let Some(ref current_claimer) = task.claimed_by {
                        if current_claimer != agent_id {
                            return Err(HiveError::TaskAlreadyClaimed {
                                task_id: task_id.to_string(),
                                claimed_by: current_claimer.clone(),
                            });
                        }
                    }
                }
            }
        }

        let new_expires = (now + chrono::Duration::seconds(ttl_secs as i64)).to_rfc3339();
        let worktree = format!("/home/drakestapleton/workspace/hive-worktrees/{}", task_id);

        conn.execute(
            "UPDATE forge_tasks SET status = 'claimed', claimed_by = ?1, expires_at = ?2, worktree_path = ?3 WHERE id = ?4",
            params![agent_id, new_expires, worktree, task_id],
        )?;

        task.status = "claimed".to_string();
        task.claimed_by = Some(agent_id.to_string());
        task.expires_at = Some(new_expires);
        task.worktree_path = Some(worktree);

        Ok(task)
    }

    pub fn heartbeat_forge_task(
        &self,
        task_id: &str,
        agent_id: &str,
        ttl_secs: u64,
    ) -> Result<(), HiveError> {
        let conn = self.conn.lock().unwrap();
        let claimed_by: Option<String> = conn
            .query_row(
                "SELECT claimed_by FROM forge_tasks WHERE id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| HiveError::TaskNotFound(task_id.to_string()))?;

        match claimed_by {
            Some(ref c) if c == agent_id => {
                let new_expires = (Utc::now() + chrono::Duration::seconds(ttl_secs as i64)).to_rfc3339();
                conn.execute(
                    "UPDATE forge_tasks SET expires_at = ?1 WHERE id = ?2",
                    params![new_expires, task_id],
                )?;
                Ok(())
            }
            Some(other) => Err(HiveError::TaskUnauthorized {
                task_id: task_id.to_string(),
                claimed_by: other,
                agent_id: agent_id.to_string(),
            }),
            None => Err(HiveError::TaskUnauthorized {
                task_id: task_id.to_string(),
                claimed_by: "unclaimed".to_string(),
                agent_id: agent_id.to_string(),
            }),
        }
    }

    pub fn reclaim_expired_leases(&self) -> Result<usize, HiveError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE forge_tasks SET status = 'open', claimed_by = NULL, expires_at = NULL, worktree_path = NULL
             WHERE status = 'claimed' AND expires_at IS NOT NULL AND expires_at < ?1",
            params![now],
        )?;
        Ok(rows)
    }

    pub fn submit_forge_task(
        &self,
        task_id: &str,
        agent_id: &str,
        _branch: &str,
        _pr_url: Option<&str>,
    ) -> Result<(), HiveError> {
        let conn = self.conn.lock().unwrap();
        let claimed_by: Option<String> = conn
            .query_row(
                "SELECT claimed_by FROM forge_tasks WHERE id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| HiveError::TaskNotFound(task_id.to_string()))?;

        match claimed_by {
            Some(ref c) if c == agent_id => {
                conn.execute(
                    "UPDATE forge_tasks SET status = 'submitted' WHERE id = ?1",
                    params![task_id],
                )?;
                Ok(())
            }
            Some(other) => Err(HiveError::TaskUnauthorized {
                task_id: task_id.to_string(),
                claimed_by: other,
                agent_id: agent_id.to_string(),
            }),
            None => Err(HiveError::TaskUnauthorized {
                task_id: task_id.to_string(),
                claimed_by: "unclaimed".to_string(),
                agent_id: agent_id.to_string(),
            }),
        }
    }

    pub fn verify_forge_task(
        &self,
        task_id: &str,
        verdict: bool,
        _notes: &str,
    ) -> Result<(), HiveError> {
        let conn = self.conn.lock().unwrap();
        let new_status = if verdict { "verified" } else { "failed" };
        let rows = conn.execute(
            "UPDATE forge_tasks SET status = ?1 WHERE id = ?2",
            params![new_status, task_id],
        )?;
        if rows == 0 {
            return Err(HiveError::TaskNotFound(task_id.to_string()));
        }
        Ok(())
    }

    pub fn list_forge_tasks(
        &self,
        project: Option<&str>,
        ring: Option<usize>,
        status: Option<&str>,
    ) -> Result<Vec<ForgeTask>, HiveError> {
        let conn = self.conn.lock().unwrap();
        let mut query = "SELECT id, comb_id, project, module, ring, q, r, title, status, claimed_by, expires_at, worktree_path, created_at FROM forge_tasks WHERE 1=1".to_string();
        let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(p) = project {
            query.push_str(" AND project = ?");
            param_vals.push(Box::new(p.to_string()));
        }
        if let Some(r) = ring {
            query.push_str(" AND ring = ?");
            param_vals.push(Box::new(r as i64));
        }
        if let Some(s) = status {
            query.push_str(" AND status = ?");
            param_vals.push(Box::new(s.to_string()));
        }

        query.push_str(" ORDER BY ring ASC, created_at ASC");

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|p| p.as_ref()).collect();

        let tasks = stmt
            .query_map(&param_refs[..], |row| {
                Ok(ForgeTask {
                    id: row.get(0)?,
                    comb_id: row.get(1)?,
                    project: row.get(2)?,
                    module: row.get(3)?,
                    ring: row.get::<_, i64>(4)? as usize,
                    q: row.get(5)?,
                    r: row.get(6)?,
                    title: row.get(7)?,
                    status: row.get(8)?,
                    claimed_by: row.get(9)?,
                    expires_at: row.get(10)?,
                    worktree_path: row.get(11)?,
                    created_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_store_genesis_and_get_cells() {
        let store = CombStore::open_in_memory().expect("open in memory store");
        let (cells, bounds) = store.get_cells().expect("get cells");
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].q, 0);
        assert_eq!(cells[0].r, 0);
        assert_eq!(cells[0].author, "AIEN Genesis");
        assert_eq!(bounds.count, 1);
        assert_eq!(bounds.min_q, 0);
        assert_eq!(bounds.max_q, 0);
    }

    #[test]
    fn test_place_comb_collision_check() {
        let store = CombStore::open_in_memory().expect("open in memory store");
        
        // Attempt to place at (0, 0) where genesis comb lives -> must trigger Collision!
        let collision_result = store.place_comb(PlaceCombInput {
            q: Some(0),
            r: Some(0),
            author: "Tester".to_string(),
            role: None,
            content: "Collision test".to_string(),
            intent: None,
            parent_id: None,
        });

        match collision_result {
            Err(HiveError::Collision { q, r, comb_id }) => {
                assert_eq!(q, 0);
                assert_eq!(r, 0);
                assert_eq!(comb_id, "comb-genesis-00000000");
            }
            other => panic!("Expected collision error, got: {:?}", other),
        }
    }

    #[test]
    fn test_place_comb_with_neighbors_linking() {
        let store = CombStore::open_in_memory().expect("open in memory store");

        // Place east neighbor of genesis (1, 0)
        let placed = store.place_comb(PlaceCombInput {
            q: Some(1),
            r: Some(0),
            author: "AIEN Subagent".to_string(),
            role: Some("researcher".to_string()),
            content: "Subagent completed research task".to_string(),
            intent: Some("join".to_string()),
            parent_id: Some("comb-genesis-00000000".to_string()),
        }).expect("place east neighbor");

        assert_eq!(placed.q, 1);
        assert_eq!(placed.r, 0);
        assert_eq!(placed.neighbors.len(), 1);
        assert_eq!(placed.neighbors[0].id, "comb-genesis-00000000");
        assert_eq!(placed.neighbors[0].direction_label, "west"); // Genesis is west of (1, 0)

        // Verify get_cells links both ways
        let (cells, bounds) = store.get_cells().expect("get cells");
        assert_eq!(cells.len(), 2);
        assert_eq!(bounds.count, 2);

        let genesis = cells.iter().find(|c| c.q == 0 && c.r == 0).unwrap();
        assert_eq!(genesis.neighbors.len(), 1);
        assert_eq!(genesis.neighbors[0].direction_label, "east");
        assert_eq!(genesis.neighbors[0].id, placed.id);
    }

    #[test]
    fn test_auto_place_comb_around_parent() {
        let store = CombStore::open_in_memory().expect("open in memory store");

        // Auto place with parent_id -> should place at first free neighbor of genesis: (1, 0) East
        let auto1 = store.place_comb(PlaceCombInput {
            q: None,
            r: None,
            author: "Subagent 1".to_string(),
            role: Some("coder".to_string()),
            content: "First code commit".to_string(),
            intent: Some("branch".to_string()),
            parent_id: Some("comb-genesis-00000000".to_string()),
        }).expect("auto place 1");

        assert_eq!(auto1.q, 1);
        assert_eq!(auto1.r, 0);

        // Auto place again -> should place at next free neighbor: (1, -1) Northeast
        let auto2 = store.place_comb(PlaceCombInput {
            q: None,
            r: None,
            author: "Subagent 2".to_string(),
            role: Some("tester".to_string()),
            content: "Tests passed".to_string(),
            intent: Some("join".to_string()),
            parent_id: Some("comb-genesis-00000000".to_string()),
        }).expect("auto place 2");

        assert_eq!(auto2.q, 1);
        assert_eq!(auto2.r, -1);
    }

    #[test]
    fn test_concurrent_auto_place_comb() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(CombStore::open_in_memory().expect("in memory store"));
        let mut handles = Vec::new();

        for i in 0..12 {
            let s = Arc::clone(&store);
            handles.push(thread::spawn(move || {
                s.place_comb(PlaceCombInput {
                    q: None,
                    r: None,
                    author: format!("Agent-{}", i),
                    role: Some("subagent".to_string()),
                    content: format!("Concurrent message {}", i),
                    intent: Some("branch".to_string()),
                    parent_id: Some("comb-genesis-00000000".to_string()),
                })
            }));
        }

        for h in handles {
            let res = h.join().expect("thread join");
            assert!(res.is_ok(), "Concurrent auto placement must never collide: {:?}", res.err());
        }

        let (cells, bounds) = store.get_cells().expect("get cells");
        assert_eq!(cells.len(), 13);
        assert_eq!(bounds.count, 13);
    }

    #[test]
    fn test_forge_project_genesis_and_tasks() {
        let store = CombStore::open_in_memory().expect("in memory store");

        let project_task = store.spawn_forge_project(
            "harvester",
            "Model Harvester Pipeline Core",
            "Extract reasoning tokens from commercial LLMs",
        ).expect("spawn forge project");

        assert_eq!(project_task.project, "harvester");
        assert_eq!(project_task.ring, 0);
        assert_eq!(project_task.status, "open");

        let module_task = store.create_forge_task(CreateForgeTaskInput {
            project: "harvester".to_string(),
            module: "openai-provider".to_string(),
            ring: 1,
            title: "Implement OpenAI provider client".to_string(),
            description: "Streaming extraction with reasoning tokens".to_string(),
            parent_comb_id: Some(project_task.comb_id.clone()),
        }).expect("create module task");

        assert_eq!(module_task.project, "harvester");
        assert_eq!(module_task.module, "openai-provider");
        assert_eq!(module_task.ring, 1);
        assert_eq!(module_task.status, "open");

        let tasks = store.list_forge_tasks(Some("harvester"), None, None).expect("list tasks");
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_forge_task_lease_heartbeat_and_reclaim() {
        let store = CombStore::open_in_memory().expect("in memory store");

        let task = store.spawn_forge_project("cortex", "Cortex Core", "Memory graph").unwrap();

        // Agent 1 claims task
        let claimed = store.claim_forge_task(&task.id, "agent-alpha", 60).expect("claim task");
        assert_eq!(claimed.status, "claimed");
        assert_eq!(claimed.claimed_by.as_deref(), Some("agent-alpha"));

        // Agent 2 attempts to claim -> fails
        let err = store.claim_forge_task(&task.id, "agent-beta", 60);
        assert!(matches!(err, Err(HiveError::TaskAlreadyClaimed { .. })));

        // Agent 1 heartbeats
        let hb = store.heartbeat_forge_task(&task.id, "agent-alpha", 120);
        assert!(hb.is_ok());

        // Agent 2 attempts unauthorized heartbeat -> fails
        let hb_err = store.heartbeat_forge_task(&task.id, "agent-beta", 120);
        assert!(matches!(hb_err, Err(HiveError::TaskUnauthorized { .. })));

        // Agent 1 submits task
        let submit = store.submit_forge_task(&task.id, "agent-alpha", "feat/cortex-core", None);
        assert!(submit.is_ok());

        // AEGIS verifies task
        let verify = store.verify_forge_task(&task.id, true, "Passes zero secret and unit tests");
        assert!(verify.is_ok());

        let final_tasks = store.list_forge_tasks(Some("cortex"), None, Some("verified")).unwrap();
        assert_eq!(final_tasks.len(), 1);
        assert_eq!(final_tasks[0].status, "verified");
    }
}

