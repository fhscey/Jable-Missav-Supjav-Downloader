use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use crate::core::model::{TaskId, TaskRecord, TaskStatus};
use crate::utils::current_timestamp;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self, DatabaseError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        log::info!("Database: opening SQLite connection at {:?}", path);
        let conn = Connection::open(path)?;

        // 开启 WAL 模式以提升并发读写性能
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn init(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                stream_url TEXT NOT NULL,
                referer TEXT,
                source_url TEXT,
                save_dir TEXT NOT NULL,
                status TEXT NOT NULL,
                completed_segments INTEGER,
                total_segments INTEGER,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                total_bytes INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;

        // 冷启动自愈：上次正在运行中或排队挂起中的任务，重置为 paused
        let affected = conn.execute(
            "UPDATE tasks SET status = 'paused', updated_at = ?1 WHERE status IN ('running', 'pending');",
            params![current_timestamp()],
        )?;
        if affected > 0 {
            log::info!(
                "Database: healed {} crashed/interrupted tasks to 'paused'",
                affected
            );
        }

        Ok(())
    }

    pub fn insert_task(&self, task: &TaskRecord) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tasks (id, title, stream_url, referer, source_url, save_dir, status, completed_segments, total_segments, downloaded_bytes, total_bytes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
            params![
                task.id,
                task.title,
                task.stream_url,
                task.referer,
                task.source_url,
                task.save_dir.to_string_lossy(),
                task.status,
                task.completed_segments.map(|v| v as i64),
                task.total_segments.map(|v| v as i64),
                task.downloaded_bytes as i64,
                task.total_bytes.map(|v| v as i64),
                task.created_at,
                task.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3;",
            params![status, current_timestamp(), id],
        )?;
        Ok(())
    }

    pub fn update_stream_url(
        &self,
        id: &TaskId,
        stream_url: &str,
        referer: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tasks SET stream_url = ?1, referer = ?2, updated_at = ?3 WHERE id = ?4;",
            params![stream_url, referer, current_timestamp(), id],
        )?;
        Ok(())
    }

    pub fn update_progress(
        &self,
        id: &TaskId,
        completed_segments: Option<usize>,
        total_segments: Option<usize>,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tasks SET completed_segments = coalesce(?1, completed_segments), total_segments = coalesce(?2, total_segments), downloaded_bytes = ?3, total_bytes = coalesce(?4, total_bytes), updated_at = ?5 WHERE id = ?6;",
            params![
                completed_segments.map(|v| v as i64),
                total_segments.map(|v| v as i64),
                downloaded_bytes as i64,
                total_bytes.map(|v| v as i64),
                current_timestamp(),
                id
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &TaskId) -> Result<Option<TaskRecord>, DatabaseError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, stream_url, referer, source_url, save_dir, status, completed_segments, total_segments, downloaded_bytes, total_bytes, created_at, updated_at
             FROM tasks WHERE id = ?1;",
        )?;
        let task = stmt
            .query_row(params![id], Self::row_to_record)
            .optional()?;
        Ok(task)
    }

    pub fn get_all_tasks(&self) -> Result<Vec<TaskRecord>, DatabaseError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, stream_url, referer, source_url, save_dir, status, completed_segments, total_segments, downloaded_bytes, total_bytes, created_at, updated_at
             FROM tasks ORDER BY created_at DESC;",
        )?;
        let rows = stmt.query_map([], Self::row_to_record)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn get_active_tasks(&self) -> Result<Vec<TaskRecord>, DatabaseError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, stream_url, referer, source_url, save_dir, status, completed_segments, total_segments, downloaded_bytes, total_bytes, created_at, updated_at
             FROM tasks
             WHERE status IN ('pending', 'paused', 'incomplete')
             ORDER BY created_at ASC;",
        )?;
        let rows = stmt.query_map([], Self::row_to_record)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn delete_task(&self, id: &TaskId) -> Result<(), DatabaseError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tasks WHERE id = ?1;", params![id])?;
        Ok(())
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<TaskRecord> {
        let id: TaskId = row.get(0)?;
        let title: String = row.get(1)?;
        let stream_url: String = row.get(2)?;
        let referer: Option<String> = row.get(3)?;
        let source_url: Option<String> = row.get(4)?;
        let save_dir_str: String = row.get(5)?;
        let status: TaskStatus = row.get(6)?;
        let completed_segments: Option<i64> = row.get(7)?;
        let total_segments: Option<i64> = row.get(8)?;
        let downloaded_bytes: i64 = row.get(9)?;
        let total_bytes: Option<i64> = row.get(10)?;
        let created_at: i64 = row.get(11)?;
        let updated_at: i64 = row.get(12)?;

        Ok(TaskRecord {
            id,
            title,
            stream_url,
            referer,
            source_url,
            save_dir: PathBuf::from(save_dir_str),
            status,
            completed_segments: completed_segments.map(|v| v.max(0) as usize),
            total_segments: total_segments.map(|v| v.max(0) as usize),
            downloaded_bytes: downloaded_bytes.max(0) as u64,
            total_bytes: total_bytes.map(|v| v.max(0) as u64),
            created_at,
            updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_crud_and_healing() {
        let db_path = std::env::temp_dir().join(format!("test_avdl_{}.db", TaskId::new_v4()));

        let db = Database::new(&db_path).unwrap();
        db.init().unwrap();

        let task_id = TaskId::new_v4();
        let record = TaskRecord {
            id: task_id,
            title: "Test Video".to_string(),
            stream_url: "https://example.com/video.m3u8".to_string(),
            referer: Some("https://example.com".to_string()),
            source_url: Some("https://example.com/detail.html".to_string()),
            save_dir: PathBuf::from("/downloads"),
            status: TaskStatus::Running,
            completed_segments: None,
            total_segments: None,
            downloaded_bytes: 0,
            total_bytes: None,
            created_at: 1000,
            updated_at: 1000,
        };

        db.insert_task(&record).unwrap();
        let fetched = db.get_task(&task_id).unwrap().unwrap();
        assert_eq!(fetched.title, "Test Video");
        assert_eq!(fetched.status, TaskStatus::Running);

        db.update_status(&task_id, TaskStatus::Paused).unwrap();
        let updated = db.get_task(&task_id).unwrap().unwrap();
        assert_eq!(updated.status, TaskStatus::Paused);

        db.update_status(&task_id, TaskStatus::Running).unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Running
        );

        db.init().unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Paused
        );

        db.update_status(&task_id, TaskStatus::Pending).unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Pending
        );
        db.init().unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Paused
        );

        db.update_status(&task_id, TaskStatus::Incomplete).unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Incomplete
        );
        db.init().unwrap();
        assert_eq!(
            db.get_task(&task_id).unwrap().unwrap().status,
            TaskStatus::Incomplete
        );
    }
}
